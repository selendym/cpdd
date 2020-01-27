# cpdd

## Summary

**cpdd** is a simple copy and deduplication tool (thus the name) written in [Rust] that uses [reflinking][COW] at the file-system level for its deduplication.


## Overview

cpdd is a copy and deduplication tool that uses reflinking for the deduplication.
cpdd recognizes directories, files, and symlinks, and copies these from the source paths to the destination directory; other file types are not supported and result in an error.
File times and permissions are preserved.

cpdd was written to allow merging of directory trees that may share content without requiring space for duplicate entries.
For example, consider two 2 TB (manual) backups that share a significant amount of content but may differ somewhat.
Say, these two should be merged on a new 3 TB storage device.
Assuming the differing content on the two backups is less than 1 TB, cpdd allows the merging.
Assume also that some of the files on either one of the backups have gone corrupt.
Although the relative paths and file sizes match, the hashes do not.
cpdd preserves both of the files (assuming `--overwrite` is not given) and renames the already existing file using the `--backup-suffix`.
This allows one to manually check, and hopefully recover, the corrupt file.

cpdd can also be used to simply deduplicate files.
All file copy operations first try to reflink the source to the destination; only if this fails is a real copy tried.
This means that cpdd copy operations not crossing file-system boundaries do not require extra space.

cpdd supports Linux and Mac, and requires that the destination file system supports reflinking.
Currently, only the following file systems should meet this requirement (see [reflink] for more details):
- Linux: [Btrfs], [XFS]
- Mac: [APFS]

(Note that only Btrfs on Linux has been tested.)
cpdd also requires that the reflink directory and the destination directory reside within the same file system, otherwise reflinking would not be possible.

By default, when `--overwrite` is not given, destination paths that already exist are renamed using the given `--backup-suffix` that defaults to `~`.
The backup renaming is recursive: if the target path exists, it is renamed, and so on.
However, if the source and the destination are determined equivalent, the source is simply skipped.
Equivalence is determined like so:
- directories: names match (that is, always)
- files: hashes match
- symlinks: link contents match

All modifying file-system operations are followed immediately by sync calls and file copy operations are followed by hash validation.
This is to provide some certainty that the operations have actually succeeded, although the downside of this approach is somewhat slower operation, especially when the source path count is large (for example, many small files).

The file deduplication works as follows:
- First, the source file is hashed and the reflink directory is checked for a matching file (the file names correspond to the hashes).
- If no such match is present, the source file is copied (or reflinked, if possible) to the reflink directory, otherwise this step is skipped.
- Finally, the matching file in the reflink directory is reflinked to the destination file.

(Note that the final step requires that the reflink directory resides within the same file system as the destination directory for reflinking to work.)
The previous implies the following:
- The reflink directory is in essence a hash-named catalog of unique regular files.
- In case any of the deduplicated files are modified, the other instances of the file are not affected, as per [copy-on-write (COW)][COW] semantics; modifications will however break the deduplication and thus additional space is required.
- The reflink directory can be removed safely afterwards, if desired.
- If deduplication is desired, different cpdd invocations should share the same reflink directory; in addition, if merging is desired, the destination directory should be shared as well.


## Installation

[Install Rust] and compile with:
- `cargo build --release`

The compiled executable will be in `target/release/cpdd`.


## Usage

```
$ cpdd --help
cpdd 0.1.0
selendym <selendym@tuta.io>
This program is a simple copy and deduplication tool

USAGE:
    cpdd [OPTIONS] <SUBCOMMAND>

FLAGS:
    -h, --help
            Prints help information

    -V, --version
            Prints version information


OPTIONS:
        --log-level <log-level>
            The log level. Possible values: `0`: off, `1`: error, `2`: warn, `3`: info (default), `4`: debug, `5`: trace

        --log-path <log-path>
            The log path.

            By default, log output is written only to stderr. If this option is set, log output is also written to the
            given path.

SUBCOMMANDS:
    copy      Copy and deduplicate source paths to the destination directory
    hash      Calculate file hashes
    help      Prints this message or the help of the given subcommand(s)
    verify    Verify reflink directory file hashes
```

```
$ cpdd copy --help
cpdd-copy 0.1.0
Copy and deduplicate source paths to the destination directory

USAGE:
    cpdd copy [FLAGS] [OPTIONS] --dst-dir <dst-dir> --reflink-dir <reflink-dir> [src-paths]...

FLAGS:
    -h, --help
            Prints help information

        --overwrite
            Overwrite existing destination paths.

            Note that existing destination directories are not overwritten but are merged or renamed, depending on the
            source file type.
        --recurse
            Recurse source directories

    -V, --version
            Prints version information


OPTIONS:
        --backup-suffix <backup-suffix>
            The backup suffix to use for renaming existing destination paths. Must not be the null string [default: ~]

    -d, --dst-dir <dst-dir>
            The destination directory

    -r, --reflink-dir <reflink-dir>
            The reflink directory. Created if nonexistent


ARGS:
    <src-paths>...
            The list of source paths to copy
```

For example:
- `$ cpdd --log-level 4 --log-path cpdd.log copy --recurse --backup-suffix '~cpdd' -r .cpdd/ -d important/ -- /mnt/important\~1/* /mnt/important\~2/*`

or alternatively:
- `$ cpdd --log-level 4 --log-path cpdd.log.1 copy --recurse --backup-suffix '~cpdd' -r .cpdd/ -d important/ -- /mnt/important\~1/*`
- `$ cpdd --log-level 4 --log-path cpdd.log.2 copy --recurse --backup-suffix '~cpdd' -r .cpdd/ -d important/ -- /mnt/important\~2/*`


[Rust]: https://www.rust-lang.org/
[Install Rust]: https://www.rust-lang.org/tools/install

[reflink]: https://docs.rs/reflink/0.1/reflink/fn.reflink.html#implementation-details-per-platform
[COW]: https://en.wikipedia.org/wiki/Copy-on-write

[Btrfs]: https://en.wikipedia.org/wiki/Btrfs
[XFS]: https://en.wikipedia.org/wiki/XFS
[APFS]: https://en.wikipedia.org/wiki/Apple_File_System
