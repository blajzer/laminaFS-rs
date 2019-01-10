#[macro_use]
extern crate bitflags;
extern crate laminafs_sys;

use std::ffi::CString;

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
	pub fn new() -> LaminaFS {
		unsafe {
			LaminaFS {
				context: laminafs_sys::lfs_context_create(&mut laminafs_sys::lfs_default_allocator)
			}
		}
	}

	pub fn new_with_capacity(work_item_queue_size: u64, work_item_pool_size: u64) -> LaminaFS {
		unsafe {
			LaminaFS {
				context: laminafs_sys::lfs_context_create_capacity(
					&mut laminafs_sys::lfs_default_allocator,
					work_item_queue_size,
					work_item_pool_size)
			}
		}
	}

	pub fn create_mount_with_permissions(&self, device_type: u32, mount_point: &str, device_path: &str, permissions: MountPermissions) -> Result<Mount, ResultCode> {
		let mut result_code: laminafs_sys::lfs_error_code_t = 0;
		let mount = unsafe { laminafs_sys::lfs_create_mount_with_permissions(
			self.context,
			device_type,
			CString::new(mount_point).unwrap().as_c_str().as_ptr(),
			CString::new(device_path).unwrap().as_c_str().as_ptr(),
			&mut result_code,
			permissions.bits()) };

		if result_code == laminafs_sys::lfs_error_code_t_LFS_OK {
			Ok(Mount {
				mount: mount,
				context: &self.context
			})
		} else {
			Err(lfs_error_to_rust(result_code))
		}
	}

	pub fn create_mount(&self, device_type: u32, mount_point: &str, device_path: &str) -> Result<Mount, ResultCode> {
		self.create_mount_with_permissions(device_type, mount_point, device_path, MountPermissions::Default)
	}

	pub fn read_file(&self, path: &str, null_terminate: bool) -> WorkItem {
		let work_item = unsafe { laminafs_sys::lfs_read_file_ctx_alloc(
    		self.context,
    		CString::new(path).unwrap().as_c_str().as_ptr(),
    		null_terminate,
			None,
    		0 as *mut std::ffi::c_void) };

		WorkItem {
			work_item: work_item,
			context: &self.context,
			write_buffer: None,
			finished: false,
			owns_buffer: true
		}
	}

	//pub fn write_file(&self, path: &str, )
}

impl Drop for LaminaFS {
	fn drop(&mut self) {
		unsafe {
			laminafs_sys::lfs_context_destroy(self.context);
		}
	}
}

pub struct Mount<'a> {
	mount: laminafs_sys::lfs_mount_t,
	context: &'a laminafs_sys::lfs_context_t
}

impl<'a> Drop for Mount<'a> {
	fn drop(&mut self) {
		unsafe {
			laminafs_sys::lfs_release_mount(*self.context, self.mount);
		}
	}
}

pub struct WorkItem<'a, 'b> {
	work_item: *mut laminafs_sys::lfs_work_item_t,
	context: &'a laminafs_sys::lfs_context_t,
	write_buffer: Option<&'b [u8]>,
	finished: bool,
	owns_buffer: bool
}

impl<'a, 'b> WorkItem<'a, 'b> {
	pub fn wait(&mut self) {
		if !self.finished {
			unsafe { laminafs_sys::lfs_wait_for_work_item(self.work_item); }
			self.finished = true;
		}
	}

	pub fn get_result(&mut self) -> ResultCode {
		self.wait();
		lfs_error_to_rust(unsafe { laminafs_sys::lfs_work_item_get_result(self.work_item) })
	}

	pub fn get_bytes(&mut self) -> usize {
		self.wait();
		(unsafe { laminafs_sys::lfs_work_item_get_bytes(self.work_item) }) as usize
	}

	pub fn get_buffer(&mut self) -> &[u8] {
		self.wait();
		let buffer_len = self.get_bytes();
		let buffer_ptr = (unsafe { laminafs_sys::lfs_work_item_get_buffer(self.work_item) }) as *mut u8;
		if buffer_ptr != 0 as *mut u8 && buffer_len > 0 {
			unsafe { std::slice::from_raw_parts(buffer_ptr, buffer_len) }
		} else {
			unsafe { std::slice::from_raw_parts(std::ptr::NonNull::dangling().as_ptr(), 0) }
		}

	}
}

impl<'a, 'b> Drop for WorkItem<'a, 'b> {
	fn drop(&mut self) {
		if self.owns_buffer {
			unsafe { laminafs_sys::lfs_work_item_free_buffer(self.work_item); }
		}
		unsafe { laminafs_sys::lfs_release_work_item(*self.context, self.work_item); }
	}
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
