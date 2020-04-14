// cpdd



// MACROS

macro_rules! rp
{
    ( $res:expr ) => (
        match $res {
            Ok( value_ ) => { value_ },
            Err( error_ ) => {
                let error_msg = format!(
                    "cpdd: main: `{}` failed: {:?}",
                    stringify!( $res ),
                    error_,
                ).replace( '\n', " " );  // FIX?: use null string

                log::error!( "{}", error_msg );

                panic!( "{}", error_msg );
            },
        }
    );
}



// CLI

#[ derive( Debug, structopt::StructOpt ) ]
#[ structopt( author ) ]
/// This program is a simple copy and deduplication tool.
struct CliArgs
{
    #[ structopt( long ) ]
    /// The log level.
    /// Possible values:
    ///     `0`: off,
    ///     `1`: error,
    ///     `2`: warn,
    ///     `3`: info (default),
    ///     `4`: debug,
    ///     `5`: trace.
    log_level: Option< u32 >,

    #[ structopt( long ) ]
    /// The log path.
    ///
    /// By default, log output is written only to stderr.
    /// If this option is set, log output is written also to the given path.
    log_path: Option< String >,

    #[ structopt( subcommand ) ]
    /// The action to be taken.
    action: Action,
}

#[ derive( Debug, structopt::StructOpt ) ]
enum Action
{
    /// Copy and deduplicate source paths to the destination directory.
    Copy{
        #[ structopt( long = "recurse" ) ]
        /// Recurse source directories.
        recurse_dirs: bool,

        #[ structopt( long = "overwrite" ) ]
        /// Overwrite existing destination paths.
        ///
        /// Note that existing destination directories are not overwritten
        /// but are merged or renamed, depending on the source file type.
        overwrite_dst: bool,

        #[ structopt( long = "skip-invalid" ) ]
        /// Skip invalid source file types.
        ///
        /// By default, invalid source file types result in an error.
        /// If this option is set, invalid file types result only in a warning.
        skip_invalid_file_types: bool,

        #[ structopt( long, default_value = "~" ) ]
        /// The backup suffix to use for renaming existing destination paths.
        /// Must not be the null string.
        backup_suffix: String,

        #[ structopt( short, long, required = true ) ]
        /// The reflink directory.
        /// Created if nonexistent.
        reflink_dir: String,

        #[ structopt( short, long, required = true ) ]
        /// The destination directory.
        dst_dir: String,

        // #[ structopt( required = true ) ]
        /// The list of source paths to copy.
        src_paths: Vec< String >,
    },

    /// Verify reflink directory file hashes.
    Verify{
        // #[ structopt( required = true ) ]
        /// The reflink directory.
        reflink_dir: String,
    },

    /// Calculate file hashes.
    Hash{
        // #[ structopt( required = true ) ]
        /// The list of source paths to calculate hashes for.
        src_paths: Vec< String >,
    },
}



// MAIN

fn main()
{
    use structopt::StructOpt;

    let cli_args = CliArgs::from_args();

    let log_level =
            match cli_args.log_level {
                Some( 0 ) => { simplelog::LevelFilter::Off },
                Some( 1 ) => { simplelog::LevelFilter::Error },
                Some( 2 ) => { simplelog::LevelFilter::Warn },
                Some( 3 ) => { simplelog::LevelFilter::Info },
                Some( 4 ) => { simplelog::LevelFilter::Debug },
                Some( 5 ) => { simplelog::LevelFilter::Trace },

                _ => { simplelog::LevelFilter::Info },
            };
    let log_config = simplelog::ConfigBuilder::new()
            .set_target_level( simplelog::LevelFilter::Error )
            .set_location_level( simplelog::LevelFilter::Error )
            .set_thread_level( simplelog::LevelFilter::Error )
            .set_time_format_str( "%FT%T%.9f%:z" )  // iso date-time, ns precision
            .add_filter_allow_str( "cpdd" )
            .build();
    match &cli_args.log_path {
        Some( log_path_ ) => {
            rp!( simplelog::CombinedLogger::init( vec![
                rp!( simplelog::TermLogger::new(
                    log_level,
                    log_config.clone(),
                    simplelog::TerminalMode::Stderr,
                ).ok_or( "Logger initialization failed." ) ),
                simplelog::WriteLogger::new(
                    log_level,
                    log_config,
                    rp!( std::fs::OpenOptions::new()
                        .write( true ).create_new( true )
                        .open( log_path_ ) ),
                ),
            ] ) );

            log::debug!(
                "Logger initialized: log_level: {:?}, log_path: {:?}",
                log_level,
                log_path_,
            );
        },
        None => {
            rp!( simplelog::TermLogger::init(
                    log_level, log_config, simplelog::TerminalMode::Stderr ) );

            log::debug!( "Logger initialized: log_level: {:?}", log_level );
        },
    };

    log::debug!( "{:?}", cli_args );

    match cli_args.action {
        Action::Copy{
            recurse_dirs,
            overwrite_dst,
            skip_invalid_file_types,
            backup_suffix,
            reflink_dir,
            dst_dir,
            src_paths,
        } => {
            match std::fs::metadata( &reflink_dir ) {
                Ok( metadata_ ) => {
                    if !metadata_.is_dir() {
                        let error_msg = format!(
                            "Invalid reflink directory file type: \
                                    not a directory: \
                                path: {:?}, \
                                type: {:?}",
                            reflink_dir,
                            metadata_.file_type(),
                        );

                        log::error!( "{}", error_msg );

                        let error = std::io::Error::new(
                                std::io::ErrorKind::InvalidInput, error_msg );

                        rp!( Err( error ) );
                    }
                },
                Err( error_ ) => {
                    match error_.kind() {
                        std::io::ErrorKind::NotFound => {
                            log::info!( "Reflink directory not found; creating." );

                            rp!( std::fs::create_dir( &reflink_dir ) );
                            rp!( rp!( std::fs::File::open( &reflink_dir ) )
                                    .sync_all() );
                        },

                        _ => { rp!( Err( error_ ) ); },
                    }
                },
            }

            let dst_metadata = rp!( std::fs::metadata( &dst_dir ) );
            if !dst_metadata.is_dir() {
                let error_msg = format!(
                    "Invalid destination directory file type: not a directory: \
                        path: {:?}, \
                        type: {:?}",
                    dst_dir,
                    dst_metadata.file_type(),
                );

                log::error!( "{}", error_msg );

                let error = std::io::Error::new(
                        std::io::ErrorKind::InvalidInput, error_msg );

                rp!( Err( error ) );
            }

            for src_path_ in src_paths {
                log::debug!( "Handling source path: {:?}", src_path_ );

                rp!( cpdd::cpdd(
                    src_path_,
                    &dst_dir,
                    &reflink_dir,
                    recurse_dirs,
                    overwrite_dst,
                    skip_invalid_file_types,
                    &backup_suffix,
                ) );
            }
        },
        Action::Verify{ reflink_dir } => {
            let reflink_dir_metadata = rp!( std::fs::metadata( &reflink_dir ) );
            if !reflink_dir_metadata.is_dir() {
                let error_msg = format!(
                    "Invalid reflink directory file type: not a directory: \
                        path: {:?}, \
                        type: {:?}",
                    reflink_dir,
                    reflink_dir_metadata.file_type(),
                );

                log::error!( "{}", error_msg );

                let error = std::io::Error::new(
                        std::io::ErrorKind::InvalidInput, error_msg );

                rp!( Err( error ) );
            }

            let mismatches = rp!( cpdd::verify_reflink_dir( &reflink_dir ) );
            if mismatches.is_empty() { println!( "No errors found." ); }
            else { println!( "Errors found:" ); }
            for path_ in mismatches { println!( "{}", path_.to_str().unwrap() ); }
        },
        Action::Hash{ src_paths } => {
            for src_path_ in src_paths {
                log::debug!( "Handling source path: {:?}", src_path_ );

                let hash = rp!( cpdd::calc_file_hash( &src_path_ ) );

                log::info!( "Result: hash: {:?}, path: {:?}", hash, src_path_ );

                println!( "{} {}", hash, src_path_ );
            }
        },
    }
}
