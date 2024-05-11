//! Contains preloading logic.

use std::{
	error::Error,
	fmt::{self, Display},
	fs, io,
	path::{Path, PathBuf},
	time::Instant,
};

use ::windows::Win32::{
	System::Com::{CoCreateInstance, CoInitialize, CreateBindCtx, CLSCTX_ALL},
	UI::Shell::{IShellItem, IThumbnailCache, SHCreateItemFromParsingName},
};
use thousands::Separable;
use windows::Win32::{
	Foundation::{BOOL, HWND},
	System::Com::IBindCtx,
	UI::Shell::{CLSID_ProgressDialog, IProgressDialog, PROGDLG_AUTOTIME, PROGDLG_NOMINIMIZE, WTS_FORCEEXTRACTION},
};
use windows_core::{w, GUID, HSTRING, PCWSTR};

/// Represents the GUID of the local thumbnail cache instance.
const LOCAL_THUMBNAIL_CACHE: GUID = GUID::from_u128(0x50ef4544_ac9f_4a8e_b21b_8a26180db13f);

/// Represents the default dimensions of a thumbnail.
const DIMENSIONS: u32 = 72;

/// Represents a preload-related error.
#[derive(Debug)]
pub enum PreloadError {
	/// Indicates the specified directory is invalid.
	InvalidDirectory(io::Error),

	/// Indicates that the contents of the directory could not be read.
	FailedToReadDirectory(io::Error),

	/// Indicates that COM failed to initialise.
	FailedToInitialiseCOM(windows_core::Error),

	/// Indicates that the progress dialog could not be created.
	FailedToCreateProgressDialog(windows_core::Error),

	/// Indicates that the bind context could not be created.
	FailedToCreateBindContext(windows_core::Error),

	/// Indicates that the thumbnail cache could not be created.
	FailedToCreateThumbnailCache(windows_core::Error),

	/// Indicates that a shell item for a particular file could not be created.
	FailedToCreateShellItem(windows_core::Error),

	/// Indicates that the progress dialog could not be shown.
	FailedToShowProgressDialog(windows_core::Error),

	/// Indicates that the progress dialog could not be updated.
	FailedToUpdateProgressDialog(windows_core::Error),

	/// Indicates that a thumbnail for a particular file could not be generated.
	FailedToGenerateThumbnail(windows_core::Error),

	/// Indicates that the progress dialog could not be hidden.
	FailedToHideProgressDialog(windows_core::Error),
}

/// Represents the result of a preload operation.
pub type PreloadResult = Result<(), PreloadError>;

impl Display for PreloadError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Self::InvalidDirectory(e) => write!(f, "invalid directory [{}]", e),
			Self::FailedToReadDirectory(e) => write!(f, "failed to read directory [{}]", e),
			Self::FailedToInitialiseCOM(e) => write!(f, "failed to initialise COM [{}]", e),
			Self::FailedToCreateProgressDialog(e) => write!(f, "failed to create progress dialog [{}]", e),
			Self::FailedToCreateThumbnailCache(e) => write!(f, "failed to create thumbnail cache [{}]", e),
			Self::FailedToCreateBindContext(e) => write!(f, "failed to create bind context [{}]", e),
			Self::FailedToShowProgressDialog(e) => write!(f, "failed to show progress dialog [{}]", e),
			Self::FailedToUpdateProgressDialog(e) => write!(f, "failed to update progress dialog [{}]", e),
			Self::FailedToCreateShellItem(e) => write!(f, "failed to create shell item [{}]", e),
			Self::FailedToGenerateThumbnail(e) => write!(f, "failed to generate thumbnail [{}]", e),
			Self::FailedToHideProgressDialog(e) => write!(f, "failed to hide progress dialog [{}]", e),
		}
	}
}

impl Error for PreloadError {}

/// Attempts to preload the specified directory.
pub fn preload<T>(dir: T) -> PreloadResult
where
	T: AsRef<Path>,
{
	println!("Preloading thumbnails for files in directory <{}>...", dir.as_ref().display());

	let dir = dir.as_ref().canonicalize().map_err(PreloadError::InvalidDirectory)?;

	let start = Instant::now();

	println!("Searching for files...");

	// Search for files and then convert them to a classic-style Windows path.
	// UNC paths do not play nice.

	let files: Vec<PathBuf> = fs::read_dir(dir)
		.map_err(PreloadError::FailedToReadDirectory)?
		.flatten()
		.map(|d| dunce::canonicalize(d.path()))
		.flatten()
		.collect();

	println!("Searched for {} files in {:#?}.", files.len(), start.elapsed());
	println!("Initialising COM...");

	// Initialise COM.

	unsafe { CoInitialize(None) }.map_err(PreloadError::FailedToInitialiseCOM)?;

	println!("Creating thumbnail cache...");

	// Initialise the IThumbnailCache instance.

	let bind_ctx = unsafe { CreateBindCtx(0) }.map_err(PreloadError::FailedToCreateBindContext)?;
	let thumb_cache: IThumbnailCache = unsafe { CoCreateInstance(&LOCAL_THUMBNAIL_CACHE, None, CLSCTX_ALL) }.map_err(PreloadError::FailedToCreateThumbnailCache)?;

	println!("Creating progress dialog...");

	// Set up the progress dialog.

	let progress_dialog: IProgressDialog = unsafe { CoCreateInstance(&CLSID_ProgressDialog, None, CLSCTX_ALL) }.map_err(PreloadError::FailedToCreateProgressDialog)?;

	unsafe {
		progress_dialog
			.SetTitle(w!("Windows Thumbnail Preloader"))
			.map_err(PreloadError::FailedToCreateProgressDialog)?;

		progress_dialog
			.SetLine(1, PCWSTR(HSTRING::from(format!("Preloading {} files", files.len().separate_with_commas())).as_ptr()), BOOL(1), None)
			.map_err(PreloadError::FailedToCreateProgressDialog)?;
	}

	let start = Instant::now();

	// Show the progress dialog.
	// We make use of the automatic time feature to provide an estimate on completion.

	unsafe { progress_dialog.StartProgressDialog(HWND(0), None, PROGDLG_AUTOTIME | PROGDLG_NOMINIMIZE, None) }.map_err(PreloadError::FailedToShowProgressDialog)?;

	println!("Preloading {} files...", files.len());

	for (index, path) in files.iter().enumerate() {
		unsafe {
			if progress_dialog.HasUserCancelled().into() {
				break;
			}
		}

		println!("Preloading file {} of {}: <{}>...", index + 1, files.len(), path.display());

		let current: u32 = index.try_into().expect("failed to convert progress index");
		let total: u32 = files.len().try_into().expect("failed to convert progress total");

		// Update the progress dialog with the current progress information.

		unsafe {
			progress_dialog
				.SetProgress(current, total)
				.map_err(PreloadError::FailedToUpdateProgressDialog)?;
		}

		unsafe {
			let line = PCWSTR(HSTRING::from(path.as_os_str()).as_ptr());

			progress_dialog
				.SetLine(2, line, BOOL(1), None)
				.map_err(PreloadError::FailedToUpdateProgressDialog)?;
		}

		// Attempt to generate the individual thumbnail.

		if let Err(err) = generate(&bind_ctx, &thumb_cache, path) {
			println!("Failed to preload file: {}.", err);
		}
	}

	println!("Preloaded files in {:#?}.", start.elapsed());

	unsafe { progress_dialog.StopProgressDialog() }.map_err(PreloadError::FailedToHideProgressDialog)?;

	Ok(())
}

/// Attempts to retrieve a thumbnail for the specified path from the specified thumbnail cache.
fn generate<T>(bind_ctx: &IBindCtx, thumb_cache: &IThumbnailCache, path: T) -> Result<(), PreloadError>
where
	T: AsRef<Path>,
{
	let pszpath = PCWSTR(HSTRING::from(path.as_ref()).as_ptr());
	let shell_item: IShellItem = unsafe { SHCreateItemFromParsingName(pszpath, bind_ctx) }.map_err(PreloadError::FailedToCreateShellItem)?;

	// Attempt to retrieve the thumbnail from the thumbnail cache.
	// This causes the thumbnail to actually be generated, even if one already exists.

	unsafe { thumb_cache.GetThumbnail(&shell_item, DIMENSIONS, WTS_FORCEEXTRACTION, None, None, None) }.map_err(PreloadError::FailedToGenerateThumbnail)?;

	Ok(())
}
