// cpdd lib



// ATTRIBUTES

// #![ allow( dead_code ) ]

#![ warn( clippy::all ) ]
#![ allow( clippy::needless_return ) ]



// CONSTANTS

const HASH_LENGTH: usize = 32;  // bytes
const BUFFER_LENGTH: usize = 1 << 28;  // bytes



// PUBLIC FUNCTIONS

pub fn cpdd< P, Q, R >(
    src_path: P,
    dst_dir: Q,
    reflink_dir: R,
    recurse_dirs: bool,
    overwrite_dst: bool,
    backup_suffix: &str,
) -> std::io::Result< () >
where
    P: AsRef< std::path::Path >,
    Q: AsRef< std::path::Path >,
    R: AsRef< std::path::Path >,
{
    log::trace!( "Begin `cpdd`." );

    let src_path = src_path.as_ref();
    let dst_dir = dst_dir.as_ref();
    let reflink_dir = reflink_dir.as_ref();

    let dst_name = src_path.file_name().ok_or_else( || {
        let error_msg = format!( "Invalid source path: {:?}", src_path );

        log::error!( "{}", error_msg );

        std::io::Error::new( std::io::ErrorKind::InvalidInput, error_msg )
    } )?;
    let dst_path = dst_dir.join( dst_name );

    log::info!( "Copying: {:?} -> {:?}", src_path, dst_path );

    let src_file_type = src_path.symlink_metadata()?.file_type();
    if src_file_type.is_dir() {
        log::debug!( "Source file type is directory." );

        cpdd_dir( src_path, &dst_path, overwrite_dst, backup_suffix )?;

        if recurse_dirs {
            log::debug!( "Recursing directory." );

            let dst_dir = &dst_path;
            for src_entry_res_ in std::fs::read_dir( src_path )? {
                let src_path = src_entry_res_?.path();
                cpdd(
                    &src_path,
                    dst_dir,
                    reflink_dir,
                    recurse_dirs,
                    overwrite_dst,
                    backup_suffix,
                )?;
            }
        }
    }
    else if src_file_type.is_file() {
        log::debug!( "Source file type is file." );

        cpdd_file(
                src_path, &dst_path, reflink_dir, overwrite_dst, backup_suffix )?;
    }
    else if src_file_type.is_symlink() {
        log::debug!( "Source file type is symlink." );

        cpdd_symlink( src_path, &dst_path, overwrite_dst, backup_suffix )?;
    }
    else {
        let error_msg = format!(
            "Invalid source file type: not a directory, file, or symlink: \
                path: {:?}, \
                type: {:?}",
            src_path,
            src_file_type,
        );

        log::error!( "{}", error_msg );

        let error = std::io::Error::new(
                std::io::ErrorKind::InvalidInput, error_msg );

        return Err( error );
    }

    copy_metadata( src_path, &dst_path )?;

    log::trace!( "End `cpdd`." );

    return Ok( () );
}

pub fn verify_reflink_dir< P >( path: P )
    -> std::io::Result< Vec< std::path::PathBuf > >
where
    P: AsRef< std::path::Path >,
{
    log::trace!( "Begin `verify_reflink_dir`." );

    let path = path.as_ref();

    log::debug!( "Verifying reflink directory: {:?}", path );

    let mut mismatches = Vec::new();
    for reflink_entry_res_ in std::fs::read_dir( path )? {
        let reflink_entry = reflink_entry_res_?;
        let reflink_path = reflink_entry.path();
        let reflink_name = reflink_entry.file_name();

        log::info!( "Verifying: {:?}", reflink_path );

        let hash: std::ffi::OsString = calc_file_hash( &reflink_path )?.into();
        if hash != reflink_name {
            log::warn!(
                "Hash mismatch: file name differs from hash: \
                    path: {:?}, \
                    name: {:?}, \
                    hash: {:?}",
                reflink_path,
                reflink_name,
                hash,
            );

            mismatches.push( reflink_path );
        }
    }

    log::trace!( "End `verify_reflink_dir`." );

    return Ok( mismatches );
}

pub fn calc_file_hash< P >( path: P ) -> std::io::Result< String >
where
    P: AsRef< std::path::Path >,
{
    use std::io::Read;

    log::trace!( "Begin `calc_file_hash`." );

    let path = path.as_ref();

    log::debug!( "Calculating file hash: {:?}", path );

    let mut file = std::fs::File::open( path )?;  // FIX?: refactor read into fn
    let mut buffer = vec![ 0; BUFFER_LENGTH ];
    let mut state = blake2b_simd::blake2bp::Params::new()
            .hash_length( HASH_LENGTH )
            .to_state();
    loop {
        match file.read( &mut buffer ) {
            Ok( 0 ) => { break; },
            Ok( count_ ) => { state.update( &buffer[ ..count_ ] ); },
            Err( error_ ) => {
                match error_.kind() {
                    std::io::ErrorKind::Interrupted => { continue; },

                    _ => { return Err( error_ ); },
                }
            },
        }
    }
    let hash = state.finalize().to_hex().as_str().to_owned();

    log::debug!( "File hash: {:?}", hash );

    log::trace!( "End `calc_file_hash`." );

    return Ok( hash );
}



// PRIVATE FUNCTIONS

fn cpdd_dir< P, Q >(
    src_path: P,
    dst_path: Q,
    overwrite_dst: bool,
    backup_suffix: &str,
) -> std::io::Result< () >
where
    P: AsRef< std::path::Path >,
    Q: AsRef< std::path::Path >,
{
    log::trace!( "Begin `cpdd_dir`." );

    let src_path = src_path.as_ref();
    let dst_path = dst_path.as_ref();

    let src_metadata = src_path.symlink_metadata()?;
    if !src_metadata.is_dir() {
        let error_msg = format!(
            "Invalid source file type: not a directory: \
                path: {:?}, \
                type: {:?}",
            src_path,
            src_metadata.file_type(),
        );

        log::error!( "{}", error_msg );

        let error = std::io::Error::new(
                std::io::ErrorKind::InvalidInput, error_msg );

        return Err( error );
    }

    log::debug!(
        "Copying directory: \
            src_path: {:?}, \
            dst_path: {:?}",
        src_path,
        dst_path,
    );

    match dst_path.symlink_metadata() {
        Ok( metadata_ ) => {
            if overwrite_dst  && !metadata_.is_dir() {
                log::info!( "Removing destination path." );

                std::fs::remove_file( dst_path )?;
            }
            else {
                if metadata_.is_dir() {
                    log::info!(
                            "Destination directory already exists; skipping." );

                    return Ok( () );
                }

                log::info!( "Renaming destination path." );

                backup_rename( dst_path, backup_suffix )?;
            }
        },
        Err( error_ ) => {
            match error_.kind() {
                std::io::ErrorKind::NotFound => {
                    log::debug!( "Destination directory not found; creating." );
                },

                _ => { return Err( error_ ); },
            }
        },
    }

    std::fs::create_dir( dst_path )?;
    std::fs::File::open( dst_path )?.sync_all()?;

    log::trace!( "End `cpdd_dir`." );

    return Ok( () );
}

fn cpdd_file< P, Q, R >(
    src_path: P,
    dst_path: Q,
    reflink_dir: R,
    overwrite_dst: bool,
    backup_suffix: &str,
) -> std::io::Result< () >
where
    P: AsRef< std::path::Path >,
    Q: AsRef< std::path::Path >,
    R: AsRef< std::path::Path >,
{
    log::trace!( "Begin `cpdd_file`." );

    let src_path = src_path.as_ref();
    let dst_path = dst_path.as_ref();
    let reflink_dir = reflink_dir.as_ref();

    let src_metadata = src_path.symlink_metadata()?;
    if !src_metadata.is_file() {
        let error_msg = format!(
            "Invalid source file type: not a file: \
                path: {:?}, \
                type: {:?}",
            src_path,
            src_metadata.file_type(),
        );

        log::error!( "{}", error_msg );

        let error = std::io::Error::new(
                std::io::ErrorKind::InvalidInput, error_msg );

        return Err( error );
    }

    let src_hash = calc_file_hash( src_path )?;
    let reflink_path = reflink_dir.join( &src_hash );

    log::debug!(
        "Copying file: \
            src_path: {:?}, \
            reflink_path: {:?}",
        src_path,
        reflink_path,
    );

    match reflink_path.symlink_metadata() {
        Ok( metadata_ ) => {
            if !metadata_.is_file() {
                let error_msg = format!(
                    "Invalid reflink file type: not a file: \
                        path: {:?}, \
                        type: {:?}",
                    reflink_path,
                    metadata_.file_type(),
                );

                log::error!( "{}", error_msg );

                let error = std::io::Error::new(
                        std::io::ErrorKind::InvalidInput, error_msg );

                return Err( error );
            }

            log::debug!( "Reflink file already exists; skipping." );
        },
        Err( error_ ) => {
            match error_.kind() {
                std::io::ErrorKind::NotFound => {
                    log::debug!( "Reflink file not found; creating." );

                    reflink_or_copy_file( src_path, &reflink_path, &src_hash )?;
                },

                _ => { return Err( error_ ); },
            }
        },
    }

    log::debug!(
        "Reflinking file: \
            reflink_path: {:?}, \
            dst_path: {:?}",
        reflink_path,
        dst_path,
    );

    match dst_path.symlink_metadata() {
        Ok( metadata_ ) => {
            if overwrite_dst  && !metadata_.is_dir() {
                log::info!( "Removing destination path." );

                std::fs::remove_file( dst_path )?;
            }
            else {
                if metadata_.is_file()
                        && metadata_.len() == src_metadata.len() {
                    let dst_hash = calc_file_hash( dst_path )?;
                    if dst_hash == src_hash {
                        // This assumes destination has already been reflinked.
                        log::info!( "Destination file already exists; skipping." );

                        return Ok( () );
                    }
                }

                log::info!( "Renaming destination path." );

                backup_rename( dst_path, backup_suffix )?;
            }
        },
        Err( error_ ) => {
            match error_.kind() {
                std::io::ErrorKind::NotFound => {
                    log::debug!( "Destination file not found; creating." );
                },

                _ => { return Err( error_ ); },
            }
        },
    }

    reflink_file( &reflink_path, dst_path )?;

    log::trace!( "End `cpdd_file`." );

    return Ok( () );
}

fn cpdd_symlink< P, Q >(
    src_path: P,
    dst_path: Q,
    overwrite_dst: bool,
    backup_suffix: &str,
) -> std::io::Result< () >
where
    P: AsRef< std::path::Path >,
    Q: AsRef< std::path::Path >,
{
    log::trace!( "Begin `cpdd_symlink`." );

    let src_path = src_path.as_ref();
    let dst_path = dst_path.as_ref();

    let src_metadata = src_path.symlink_metadata()?;
    if !src_metadata.file_type().is_symlink() {
        let error_msg = format!(
            "Invalid source file type: not a symlink: \
                path: {:?}, \
                type: {:?}",
            src_path,
            src_metadata.file_type(),
        );

        log::error!( "{}", error_msg );

        let error = std::io::Error::new(
                std::io::ErrorKind::InvalidInput, error_msg );

        return Err( error );
    }

    log::debug!(
        "Copying symlink: \
            src_path: {:?}, \
            dst_path: {:?}",
        src_path,
        dst_path,
    );

    let src_link;
    match dst_path.symlink_metadata() {
        Ok( metadata_ ) => {
            src_link = src_path.read_link()?;
            if overwrite_dst  && !metadata_.is_dir() {
                log::info!( "Removing destination path." );

                std::fs::remove_file( dst_path )?;
            }
            else {
                if metadata_.file_type().is_symlink() {
                    let dst_link = dst_path.read_link()?;
                    if dst_link == src_link {
                        log::info!(
                                "Destination symlink already exists; skipping." );

                        return Ok( () );
                    }
                }

                log::info!( "Renaming destination path." );

                backup_rename( dst_path, backup_suffix )?;
            }
        },
        Err( error_ ) => {
            match error_.kind() {
                std::io::ErrorKind::NotFound => {
                    log::debug!( "Destination symlink not found; creating." );

                    src_link = src_path.read_link()?;
                },

                _ => { return Err( error_ ); },
            }
        },
    }

    std::os::unix::fs::symlink( &src_link, dst_path )?;
    sync_symlink( dst_path )?;

    log::trace!( "End `cpdd_symlink`." );

    return Ok( () );
}

fn backup_rename< P >( path: P, suffix: &str )
    -> std::io::Result< std::path::PathBuf >
where
    P: AsRef< std::path::Path >,
{
    log::trace!( "Begin `backup_rename`." );

    let path = path.as_ref();

    assert!( !suffix.is_empty() );  // FIX?: change to error

    let mut backup_name = path.file_name().ok_or_else( || {
        let error_msg = format!( "Invalid path: {:?}", path );

        log::error!( "{}", error_msg );

        std::io::Error::new( std::io::ErrorKind::InvalidInput, error_msg )
    } )?.to_owned();
    backup_name.push( suffix );
    let backup_path = path.with_file_name( backup_name );

    log::debug!(
        "Renaming path: \
            path: {:?}, \
            backup_path: {:?}",
        path,
        backup_path,
    );

    match backup_path.symlink_metadata() {
        Ok( _ ) => {
            log::info!( "Backup path exists; renaming." );

            backup_rename( &backup_path, suffix )?;
        },
        Err( error_ ) => {
            match error_.kind() {
                std::io::ErrorKind::NotFound => {},

                _ => { return Err( error_ ); },
            }
        },
    }

    std::fs::rename( path, &backup_path )?;
    if backup_path.symlink_metadata()?.file_type().is_symlink() {
        sync_symlink( &backup_path )?;
    }
    else { std::fs::File::open( &backup_path )?.sync_all()?; }

    log::trace!( "End `backup_rename`." );

    return Ok( backup_path );
}

fn reflink_or_copy_file< P, Q >(
    src_path: P,
    dst_path: Q,
    src_hash: &str,
) -> std::io::Result< () >
where
    P: AsRef< std::path::Path >,
    Q: AsRef< std::path::Path >,
{
    log::trace!( "Begin `reflink_or_copy_file`." );

    let src_path = src_path.as_ref();
    let dst_path = dst_path.as_ref();

    // // FIX?: remove
    // if dst_path.exists()
    //         && src_path.canonicalize()? == dst_path.canonicalize()? {
    //     let error_msg = format!(
    //         "Invalid destination path: source resolves to the same path: \
    //             src_path: {:?}, \
    //             dst_path: {:?}, \
    //             resolved_path: {:?}",
    //         src_path,
    //         dst_path,
    //         src_path.canonicalize()?,
    //     );

    //     log::error!( "{}", error_msg );

    //     let error = std::io::Error::new(
    //             std::io::ErrorKind::InvalidInput, error_msg );

    //     return Err( error );
    // }

    log::debug!(
        "Reflinking or copying file: \
            src_path: {:?}, \
            dst_path: {:?}",
        src_path,
        dst_path,
    );

    if reflink::reflink( src_path, dst_path ).is_ok() {
        log::debug!( "Reflinking succeeded." );

        std::fs::File::open( dst_path )?.sync_all()?;

        return Ok( () );
    }

    log::debug!( "Reflinking failed; copying file." );

    std::fs::copy( src_path, dst_path )?;
    std::fs::File::open( dst_path )?.sync_all()?;

    log::debug!( "Verifying destination file hash: src_hash: {:?}", src_hash );

    let dst_hash = calc_file_hash( dst_path )?;
    if dst_hash != src_hash {
        let error_msg = format!(
            "File copy failed: hash mismatch: \
                src_path: {:?}, \
                dst_path: {:?}, \
                src_hash: {:?}, \
                dst_hash: {:?}",
            src_path,
            dst_path,
            src_hash,
            dst_hash,
        );

        log::error!( "{}", error_msg );

        let error = std::io::Error::new( std::io::ErrorKind::Other, error_msg );

        return Err( error );
    }

    log::trace!( "End `reflink_or_copy_file`." );

    return Ok( () );
}

fn reflink_file< P, Q >(
    src_path: P,
    dst_path: Q,
) -> std::io::Result< () >
where
    P: AsRef< std::path::Path >,
    Q: AsRef< std::path::Path >,
{
    log::trace!( "Begin `reflink_file`." );

    let src_path = src_path.as_ref();
    let dst_path = dst_path.as_ref();

    log::debug!(
        "Reflinking file: \
            src_path: {:?}, \
            dst_path: {:?}",
        src_path,
        dst_path,
    );

    reflink::reflink( src_path, dst_path )?;
    std::fs::File::open( dst_path )?.sync_all()?;

    log::trace!( "End `reflink_file`." );

    return Ok( () );
}

fn copy_metadata< P, Q >(
    src_path: P,
    dst_path: Q,
) -> std::io::Result< () >
where
    P: AsRef< std::path::Path >,
    Q: AsRef< std::path::Path >,
{
    log::trace!( "Begin `copy_metadata`." );

    let src_path = src_path.as_ref();
    let dst_path = dst_path.as_ref();

    let src_metadata = src_path.symlink_metadata()?;
    let dst_metadata = dst_path.symlink_metadata()?;
    let src_type = src_metadata.file_type();
    let dst_type = dst_metadata.file_type();
    if src_type.is_dir() && !dst_type.is_dir()
            || src_type.is_file() && !dst_type.is_file()
            || src_type.is_symlink() && !dst_type.is_symlink() {
        // This assumes source is not some other type.
        let error_msg = format!(
            "Invalid destination file type: source file type differs: \
                src_path: {:?}, \
                dst_path: {:?}, \
                src_type: {:?}, \
                dst_type: {:?}",
            src_path,
            dst_path,
            src_type,
            dst_type,
        );

        log::error!( "{}", error_msg );

        let error = std::io::Error::new(
                std::io::ErrorKind::InvalidInput, error_msg );

        return Err( error );
    }

    log::debug!(
        "Copying metadata: \
            src_path: {:?}, \
            dst_path: {:?}",
        src_path,
        dst_path,
    );

    if !src_metadata.file_type().is_symlink() {
        std::fs::set_permissions( dst_path, src_metadata.permissions() )?;
    }
    filetime::set_symlink_file_times(
        dst_path,
        filetime::FileTime::from_last_access_time( &src_metadata ),
        filetime::FileTime::from_last_modification_time( &src_metadata ),
    )?;
    if dst_metadata.file_type().is_symlink() {
        sync_symlink( dst_path )?;
    }
    else { std::fs::File::open( dst_path )?.sync_all()?; }

    log::trace!( "End `copy_metadata`." );

    return Ok( () );
}

fn sync_symlink< P >( path: P ) -> std::io::Result< () >
where
    P: AsRef< std::path::Path >,
{
    log::trace!( "Begin `sync_symlink`." );

    let path = path.as_ref();

    let metadata = path.symlink_metadata()?;
    if !metadata.file_type().is_symlink() {
        let error_msg = format!(
            "Invalid file type: not a symlink: \
                path: {:?}, \
                type: {:?}",
            path,
            metadata.file_type(),
        );

        log::error!( "{}", error_msg );

        let error = std::io::Error::new(
                std::io::ErrorKind::InvalidInput, error_msg );

        return Err( error );
    }

    let mut abs_path = std::env::current_dir()?;
    abs_path.push( path );
    let parent_path = abs_path.parent().unwrap();

    log::debug!( "Syncing symlink parent: {:?}", parent_path );

    std::fs::File::open( parent_path )?.sync_all()?;

    log::trace!( "End `sync_symlink`." );

    return Ok( () );
}
