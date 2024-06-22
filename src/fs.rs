#[cfg(target_os = "ios")]
use crate::native::ios;

#[derive(Debug)]
pub enum Error {
    IOError(std::io::Error),
    DownloadFailed,
	AndroidAssetLoadingError,
	AndroidInternalStorageError,
    /// MainBundle pathForResource returned null
    IOSAssetNoSuchFile,
    /// NSData dataWithContentsOfFile or data.bytes are null
    IOSAssetNoData,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            _ => write!(f, "Error: {:?}", self),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::IOError(e)
    }
}

pub type Response = Result<Vec<u8>, Error>;

/// Filesystem path on desktops or HTTP URL in WASM
/// Used for loading static files like assets
pub fn load_file<F: Fn(Response) + 'static>(path: &str, on_loaded: F) {
    #[cfg(target_arch = "wasm32")]
    wasm::load_file(path, on_loaded);

    #[cfg(target_os = "android")]
    load_asset_android(path, on_loaded);

    #[cfg(target_os = "ios")]
    ios::load_file(path, on_loaded);

    #[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
    load_file_desktop(path, on_loaded);
}

/// Assets are not writable
pub fn save_file<F: Fn(bool) + 'static>(path: &str, data: &[u8], on_saved: F) {
	#[cfg(target_os = "android")]
	write_internal_storage_android(path, data, on_saved);
	
	#[cfg(not(target_os = "android"))]
	unimplemented!("save_file is not implemented for this platform");
}


/// Used for internal storage on android, use load_file() instead for assets.
pub fn read_file<F: Fn(Response) + 'static>(path: &str, on_loaded: F) {
	#[cfg(target_os = "android")]
	read_internal_storage_android(path, on_loaded);
	
	#[cfg(not(target_os = "android"))]
	unimplemented!("read_file is not implemented for this platform");
}



#[cfg(target_os = "android")]
fn load_asset_android<F: Fn(Response)>(path: &str, on_loaded: F) {
    fn load_file_sync(path: &str) -> Response {
        use crate::native;

        let filename = std::ffi::CString::new(path).unwrap();

        let mut data: native::android_asset = unsafe { std::mem::zeroed() };

        unsafe { native::android::load_asset(filename.as_ptr(), &mut data as _) };

        if data.content.is_null() == false {
            let slice =
                unsafe { std::slice::from_raw_parts(data.content, data.content_length as _) };
            let response = slice.iter().map(|c| *c as _).collect::<Vec<_>>();
            Ok(response)
        } else {
            Err(Error::AndroidAssetLoadingError)
        }
    }

    let response = load_file_sync(path);

    on_loaded(response);
}

#[cfg(target_os = "android")]
fn read_internal_storage_android<F: Fn(Response)>(path: &str, on_loaded: F) {
	fn read_file_sync(path: &str) -> Response {
		use crate::native;
		
		let filename = std::ffi::CString::new(path).unwrap();
		
		let mut data: native::android_asset = unsafe { std::mem::zeroed() };
		
		unsafe { native::android::read_internal_storage(filename.as_ptr(), &mut data as _) };
		
		if data.content.is_null() == false {
			let slice =
				unsafe { std::slice::from_raw_parts(data.content, data.content_length as _) };
			let response = slice.iter().map(|c| *c as _).collect::<Vec<_>>();
			Ok(response)
		} else {
			Err(Error::AndroidInternalStorageError)
		}
	}
	
	let response = read_file_sync(path);
	
	on_loaded(response);
}

#[cfg(target_os = "android")]
fn write_internal_storage_android<F: Fn(bool)>(path: &str, data: &[u8], on_written: F) {
	fn write_file_sync(path: &str, data: &[u8]) -> bool {
		use crate::native;
		
		let filename = std::ffi::CString::new(path).unwrap();
		
		unsafe { native::android::write_internal_storage(filename.as_ptr(), data.as_ptr(), data.len()) }
	}
	
	let success = write_file_sync(path, data);
	
	on_written(success);
}



#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::Response;
    use crate::native;

    use std::{cell::RefCell, collections::HashMap, thread_local};

    thread_local! {
        static FILES: RefCell<HashMap<u32, Box<dyn Fn(Response)>>> = RefCell::new(HashMap::new());
    }

    #[no_mangle]
    pub extern "C" fn file_loaded(file_id: u32) {
        use super::Error;
        use native::wasm::fs;

        FILES.with(|files| {
            let mut files = files.borrow_mut();
            let callback = files
                .remove(&file_id)
                .unwrap_or_else(|| panic!("Unknown file loaded!"));
            let file_len = unsafe { fs::fs_get_buffer_size(file_id) };
            if file_len == -1 {
                callback(Err(Error::DownloadFailed));
            } else {
                let mut buffer = vec![0; file_len as usize];
                unsafe { fs::fs_take_buffer(file_id, buffer.as_mut_ptr(), file_len as u32) };

                callback(Ok(buffer));
            }
        })
    }

    pub fn load_file<F: Fn(Response) + 'static>(path: &str, on_loaded: F) {
        use native::wasm::fs;
        use std::ffi::CString;

        let url = CString::new(path).unwrap();
        let file_id = unsafe { fs::fs_load_file(url.as_ptr(), url.as_bytes().len() as u32) };
        FILES.with(|files| {
            let mut files = files.borrow_mut();
            files.insert(file_id, Box::new(on_loaded));
        });
    }
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
fn load_file_desktop<F: Fn(Response)>(path: &str, on_loaded: F) {
    fn load_file_sync(path: &str) -> Response {
        use std::fs::File;
        use std::io::Read;

        let mut response = vec![];
        let mut file = File::open(path)?;
        file.read_to_end(&mut response)?;
        Ok(response)
    }

    let response = load_file_sync(path);

    on_loaded(response);
}
