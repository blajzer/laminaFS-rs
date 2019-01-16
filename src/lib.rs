#[macro_use]
extern crate bitflags;
extern crate laminafs_sys;

use std::ffi::CString;
use std::ptr::NonNull;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(PartialEq, Eq)]
pub enum ResultCode {
	Ok,
	NotFound,
	InvalidDevice,
	AlreadyExists,
	OutOfSpace,
	PermissionsError,
	Unsupported,
	GenericError
}

bitflags! {
	pub struct MountPermissions: u32 {
		const All = laminafs_sys::lfs_mount_permissions_t_LFS_MOUNT_ALL_PERMISSIONS;
		const CreateDir = laminafs_sys::lfs_mount_permissions_t_LFS_MOUNT_CREATE_DIR;
		const Default = laminafs_sys::lfs_mount_permissions_t_LFS_MOUNT_DEFAULT;
		const DeleteDir = laminafs_sys::lfs_mount_permissions_t_LFS_MOUNT_DELETE_DIR;
		const DeleteFile = laminafs_sys::lfs_mount_permissions_t_LFS_MOUNT_DELETE_FILE;
		const Read = laminafs_sys::lfs_mount_permissions_t_LFS_MOUNT_READ;
		const Write = laminafs_sys::lfs_mount_permissions_t_LFS_MOUNT_WRITE;
		const WriteFile = laminafs_sys::lfs_mount_permissions_t_LFS_MOUNT_WRITE_FILE;
	}
}

fn lfs_error_to_rust(error: laminafs_sys::lfs_error_code_t) -> ResultCode {
	match error {
		laminafs_sys::lfs_error_code_t_LFS_ALREADY_EXISTS => ResultCode::AlreadyExists,
		laminafs_sys::lfs_error_code_t_LFS_GENERIC_ERROR => ResultCode::GenericError,
		laminafs_sys::lfs_error_code_t_LFS_INVALID_DEVICE => ResultCode::InvalidDevice,
		laminafs_sys::lfs_error_code_t_LFS_NOT_FOUND => ResultCode::NotFound,
		laminafs_sys::lfs_error_code_t_LFS_OK => ResultCode::Ok,
		laminafs_sys::lfs_error_code_t_LFS_OUT_OF_SPACE => ResultCode::OutOfSpace,
		laminafs_sys::lfs_error_code_t_LFS_PERMISSIONS_ERROR => ResultCode::PermissionsError,
		laminafs_sys::lfs_error_code_t_LFS_UNSUPPORTED => ResultCode::Unsupported,
		_ => panic!("Unexpected error code from Lamina {}", error)
	}
}


pub struct LaminaFS {
	context: laminafs_sys::lfs_context_t
}

impl LaminaFS {
	pub fn new() -> Arc<LaminaFS> {
		Arc::new(LaminaFS {
			context: unsafe { laminafs_sys::lfs_context_create(&mut laminafs_sys::lfs_default_allocator) }
		})
	}

	pub fn new_with_capacity(work_item_queue_size: u64, work_item_pool_size: u64) -> Arc<LaminaFS> {
		Arc::new(LaminaFS {
			context: unsafe { laminafs_sys::lfs_context_create_capacity(
				&mut laminafs_sys::lfs_default_allocator,
				work_item_queue_size,
				work_item_pool_size) }
		})
	}

	pub fn create_mount_with_permissions(&self, device_type: u32, mount_point: &str, device_path: &str, permissions: MountPermissions) -> Result<Mount, ResultCode> {
		let mut result_code: laminafs_sys::lfs_error_code_t = 0;
		let mount_point = CString::new(mount_point).unwrap();
		let device_path = CString::new(device_path).unwrap();

		let mount = unsafe { laminafs_sys::lfs_create_mount_with_permissions(
			self.context,
			device_type,
			mount_point.as_c_str().as_ptr(),
			device_path.as_c_str().as_ptr(),
			&mut result_code,
			permissions.bits()) };

		if result_code == laminafs_sys::lfs_error_code_t_LFS_OK {
			Ok(Mount {
				mount: mount,
				context: self.context
			})
		} else {
			Err(lfs_error_to_rust(result_code))
		}
	}

	pub fn create_mount(&self, device_type: u32, mount_point: &str, device_path: &str) -> Result<Mount, ResultCode> {
		self.create_mount_with_permissions(device_type, mount_point, device_path, MountPermissions::Default)
	}

	pub fn append_file(&self, path: &str, buffer: Arc<[u8]>) -> Arc<Mutex<WorkItem>> {
		let path = CString::new(path).unwrap();
		let work_item = unsafe { laminafs_sys::lfs_append_file(
			self.context,
			path.as_c_str().as_ptr(),
			buffer.as_ptr() as *const std::ffi::c_void,
			buffer.len() as u64,
			None,
			0 as *mut std::ffi::c_void) };

		Arc::new(Mutex::new(WorkItem {
			work_item: WorkItemPtr::new(work_item),
			context: self.context,
			write_buffer: Some(buffer),
			finished: false,
			owns_buffer: false
		}))
	}

	pub fn read_file(&self, path: &str, null_terminate: bool) -> Arc<Mutex<WorkItem>> {
		let path = CString::new(path).unwrap();
		let work_item = unsafe { laminafs_sys::lfs_read_file_ctx_alloc(
    		self.context,
    		path.as_c_str().as_ptr(),
    		null_terminate,
			None,
    		0 as *mut std::ffi::c_void) };

		Arc::new(Mutex::new(WorkItem {
			work_item: WorkItemPtr::new(work_item),
			context: self.context,
			write_buffer: None,
			finished: false,
			owns_buffer: true
		}))
	}

	pub fn read_file_segment(&self, path: &str, offset: u64, max_bytes: u64, null_terminate: bool) -> Arc<Mutex<WorkItem>> {
		let path = CString::new(path).unwrap();
		let work_item = unsafe { laminafs_sys::lfs_read_file_segment_ctx_alloc(
    		self.context,
    		path.as_c_str().as_ptr(),
			offset,
			max_bytes,
    		null_terminate,
			None,
    		0 as *mut std::ffi::c_void) };

		Arc::new(Mutex::new(WorkItem {
			work_item: WorkItemPtr::new(work_item),
			context: self.context,
			write_buffer: None,
			finished: false,
			owns_buffer: true
		}))
	}

	pub fn write_file(&self, path: &str, buffer: Arc<[u8]>) -> Arc<Mutex<WorkItem>> {
		let path = CString::new(path).unwrap();
		let work_item = unsafe { laminafs_sys::lfs_write_file(
			self.context,
			path.as_c_str().as_ptr(),
			buffer.as_ptr() as *const std::ffi::c_void,
			buffer.len() as u64,
			None,
			0 as *mut std::ffi::c_void) };

		Arc::new(Mutex::new(WorkItem {
			work_item: WorkItemPtr::new(work_item),
			context: self.context,
			write_buffer: Some(buffer),
			finished: false,
			owns_buffer: false
		}))
	}

	pub fn write_file_segment(&self, path: &str, offset: u64, buffer: Arc<[u8]>) -> Arc<Mutex<WorkItem>> {
		let path = CString::new(path).unwrap();
		let work_item = unsafe { laminafs_sys::lfs_write_file_segment(
			self.context,
			path.as_c_str().as_ptr(),
			offset,
			buffer.as_ptr() as *const std::ffi::c_void,
			buffer.len() as u64,
			None,
			0 as *mut std::ffi::c_void) };

		Arc::new(Mutex::new(WorkItem {
			work_item: WorkItemPtr::new(work_item),
			context: self.context,
			write_buffer: Some(buffer),
			finished: false,
			owns_buffer: false
		}))
	}

	pub fn create_dir(&self, path: &str) -> Arc<Mutex<WorkItem>> {
		let path = CString::new(path).unwrap();
		let work_item = unsafe { laminafs_sys::lfs_create_dir(
			self.context,
			path.as_c_str().as_ptr(),
			None,
			0 as *mut std::ffi::c_void) };

		Arc::new(Mutex::new(WorkItem {
			work_item: WorkItemPtr::new(work_item),
			context: self.context,
			write_buffer: None,
			finished: false,
			owns_buffer: false
		}))
	}

	pub fn delete_dir(&self, path: &str) -> Arc<Mutex<WorkItem>> {
		let path = CString::new(path).unwrap();
		let work_item = unsafe { laminafs_sys::lfs_delete_dir(
			self.context,
			path.as_c_str().as_ptr(),
			None,
			0 as *mut std::ffi::c_void) };

		Arc::new(Mutex::new(WorkItem {
			work_item: WorkItemPtr::new(work_item),
			context: self.context,
			write_buffer: None,
			finished: false,
			owns_buffer: false
		}))
	}

	pub fn delete_file(&self, path: &str) -> Arc<Mutex<WorkItem>> {
		let path = CString::new(path).unwrap();
		let work_item = unsafe { laminafs_sys::lfs_delete_file(
			self.context,
			path.as_c_str().as_ptr(),
			None,
			0 as *mut std::ffi::c_void) };

		Arc::new(Mutex::new(WorkItem {
			work_item: WorkItemPtr::new(work_item),
			context: self.context,
			write_buffer: None,
			finished: false,
			owns_buffer: false
		}))
	}

	pub fn file_exists(&self, path: &str) -> Arc<Mutex<WorkItem>> {
		let path = CString::new(path).unwrap();
		let work_item = unsafe { laminafs_sys::lfs_file_exists(
			self.context,
			path.as_c_str().as_ptr(),
			None,
			0 as *mut std::ffi::c_void) };

		Arc::new(Mutex::new(WorkItem {
			work_item: WorkItemPtr::new(work_item),
			context: self.context,
			write_buffer: None,
			finished: false,
			owns_buffer: false
		}))
	}
}

impl Drop for LaminaFS {
	fn drop(&mut self) {
		unsafe {
			laminafs_sys::lfs_context_destroy(self.context);
		}
	}
}

pub struct Mount {
	mount: laminafs_sys::lfs_mount_t,
	context: laminafs_sys::lfs_context_t
}

impl Drop for Mount {
	fn drop(&mut self) {
		unsafe {
			laminafs_sys::lfs_release_mount(self.context, self.mount);
		}
	}
}

// Internal struct used for assuring Rust that work items are Send+Sync
struct WorkItemPtr {
	ptr: NonNull<laminafs_sys::lfs_work_item_t>
}

unsafe impl Send for WorkItemPtr {}
unsafe impl Sync for WorkItemPtr {}

impl WorkItemPtr {
	fn new(ptr: *mut laminafs_sys::lfs_work_item_t) -> WorkItemPtr {
		WorkItemPtr {
			ptr: NonNull::new(ptr).unwrap()
		}
	}
}

pub struct WorkItem {
	work_item: WorkItemPtr,
	context: laminafs_sys::lfs_context_t,
	write_buffer: Option<Arc<[u8]>>,
	finished: bool,
	owns_buffer: bool
}

impl WorkItem {
	pub fn wait(&mut self) {
		if !self.finished {
			unsafe { laminafs_sys::lfs_wait_for_work_item(self.work_item.ptr.as_ptr()); }
			self.finished = true;
		}
	}

	pub fn get_result(&mut self) -> ResultCode {
		self.wait();
		lfs_error_to_rust(unsafe { laminafs_sys::lfs_work_item_get_result(self.work_item.ptr.as_ptr()) })
	}

	pub fn get_bytes(&mut self) -> usize {
		self.wait();
		(unsafe { laminafs_sys::lfs_work_item_get_bytes(self.work_item.ptr.as_ptr()) }) as usize
	}

	pub fn get_buffer(&mut self) -> &[u8] {
		self.wait();

		let buffer_len = self.get_bytes();
		let buffer_ptr = (unsafe { laminafs_sys::lfs_work_item_get_buffer(self.work_item.ptr.as_ptr()) }) as *mut u8;
		if buffer_ptr != 0 as *mut u8 && buffer_len > 0 {
			unsafe { std::slice::from_raw_parts(buffer_ptr, buffer_len) }
		} else {
			unsafe { std::slice::from_raw_parts(std::ptr::NonNull::dangling().as_ptr(), 0) }
		}

	}
}

impl Drop for WorkItem {
	fn drop(&mut self) {
		self.wait();

		if self.owns_buffer {
			unsafe { laminafs_sys::lfs_work_item_free_buffer(self.work_item.ptr.as_ptr()); }
		}
		unsafe { laminafs_sys::lfs_release_work_item(self.context, self.work_item.ptr.as_ptr()); }
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::thread;

	#[test]
	fn read_test() {
		let fs = LaminaFS::new();
		let mount = fs.create_mount(0, "/", "./");
		let work = fs.read_file("/Cargo.lock", true);

		let t = thread::spawn(move || {
			let mut item_inner = work.lock().unwrap();
			assert!(item_inner.get_result() == ResultCode::Ok);
			assert!(item_inner.get_bytes() > 0);
			assert!(item_inner.get_bytes() == item_inner.get_buffer().len());

			let contents = std::str::from_utf8(item_inner.get_buffer()).unwrap();
		});
		t.join();
	}
}
