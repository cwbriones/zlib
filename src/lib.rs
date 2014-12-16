#![feature(globs)]
#![feature(macro_rules)]
#![allow(dead_code)]
#![allow(unused_variables)]

#![feature(phase)]
#[phase(plugin, link)] extern crate log;

extern crate crc32;

pub macro_rules! Tracevv {
    ($($arg:tt)*) => {
        if cfg!(not(ndebug)) {
            // println!($($arg)*)
        }
    }
}

mod adler32;
pub mod inflate;
mod statictrees;
mod treedefs;
mod deflate;

// From deflate.h

// The three kinds of block type
const STORED_BLOCK: uint = 0;
const STATIC_TREES: uint = 1;
const DYN_TREES: uint = 2;

pub const PRESET_DICT: uint = 0x20; /* preset dictionary flag in zlib header */

#[deriving(Copy,Show,Eq,PartialEq)]
pub enum WrapKind {
    Zlib,
    Gzip
}

pub struct ZStream {
    pub next_in: uint,          // index of next input byte, within input_buffer (passed elsewhere)
    pub avail_in: uint,         // number of bytes available at next_in
    pub total_in: u64,         // total number of input bytes read so far
    pub next_out: uint,         // position within output_buffer where to write the next byte
    pub avail_out: uint,        // remaining free space at next_out
    pub total_out: u64,        // total number of bytes output so far
    pub msg: Option<String>,    // last error message, if any
    pub data_type :uint,        // best guess about the data type: binary or text
    pub adler: u32              // adler32 value of the uncompressed data
}

/// gzip header information passed to and from zlib routines.  See RFC 1952
/// for more details on the meanings of these fields.
pub struct GZipHeader {
    pub text: bool,                     // true if compressed data believed to be text
    pub time: u32,                      // modification time
    pub xflags: u32,                    // extra flags (not used when writing a gzip file)
    pub os: u32,                        // operating system
    pub extra_len: uint,                // length of the 'extra' data, in bytes
    pub extra: Option<Box<Vec<u8>>>,    // extra field data, if any
//    pub name_len: uint,                 // length of the 'name' data, in bytes (not chars!)
    pub name: Option<Box<String>>,      // filename, if any
//    pub comm_len: uint,                 // length of the 'comment' data, in bytes (not chars!)
    pub comment: Option<Box<String>>,   // comment string, if any
    pub hcrc: bool,                     // true if there was or will be a header crc
    pub done: bool,                     // true when done reading gzip header (not used when writing a gzip file)
}

impl GZipHeader {
    fn new() -> GZipHeader {
        GZipHeader {
            text: false,
            time: 0,
            xflags: 0,
            os: 0,
            extra: None,
            extra_len: 0,
            name: None,
            comment: None,
            hcrc: false,
            done: false
        }
    }
}

/*
     The application must update next_in and avail_in when avail_in has dropped
   to zero.  It must update next_out and avail_out when avail_out has dropped
   to zero.  All other fields are set by the compression
   library and must not be updated by the application.

     The fields total_in and total_out can be used for statistics or progress
   reports.  After compression, total_in holds the total size of the
   uncompressed data and may be saved for use in the decompressor (particularly
   if the decompressor wants to decompress everything in a single step).
*/

/* constants */

/* Allowed flush values; see deflate() and inflate() below for details */
#[deriving(Copy,Show,PartialEq,Eq)]
pub enum Flush {
    None = 0,
    PartialFlush = 1,
    SyncFlush = 2,
    FullFlush = 3,
    Finish = 4,
    Block = 5,
    Trees = 6
}

/*
pub const Z_NO_FLUSH     : u32 = 0;
pub const Z_PARTIAL_FLUSH: u32 = 1;
pub const Z_SYNC_FLUSH   : u32 = 2;
pub const Z_FULL_FLUSH   : u32 = 3;
pub const Z_FINISH       : u32 = 4;
pub const Z_BLOCK        : u32 = 5;
pub const Z_TREES        : u32 = 6;
*/

#[deriving(Copy,PartialEq,Eq)]
pub enum ZERR {
    Ok              = 0,        // Z_OK            = 0,
    StreamEnd       = 1,        // Z_STREAM_END    = 1,
    NeedDict        = 2,        // Z_NEED_DICT     = 2,
    Errno           = -1,        // Z_ERRNO         = -1,
    StreamError     = -2,        // Z_STREAM_ERROR  = -2,
    DataError       = -3,        // Z_DATA_ERROR    = -3,
//    MemError        = -4,        // Z_MEM_ERROR     = -4,
//    BufError        = -5,        // Z_BUF_ERROR     = -5,
//    VersionError    = -6,        // Z_VERSION_ERROR = -6,
    // Return codes for the compression/decompression functions. Negative values
    // are errors, positive values are used for special but normal events.
}

/* compression levels */
pub const Z_NO_COMPRESSION     : int = 0;
pub const Z_BEST_SPEED         : int = 1;
pub const Z_BEST_COMPRESSION   : int = 9;
pub const Z_DEFAULT_COMPRESSION: int = -1;

pub const Z_FILTERED            :uint = 1;
pub const Z_HUFFMAN_ONLY        :uint = 2;
pub const Z_RLE                 :uint = 3;
pub const Z_FIXED               :uint = 4;
pub const Z_DEFAULT_STRATEGY    :uint = 0;
/* compression strategy; see deflateInit2() below for details */

pub const Z_BINARY   :uint = 0;
pub const Z_TEXT     :uint = 1;
pub const Z_ASCII    :uint = Z_TEXT;   /* for compatibility with 1.2.2 and earlier */
pub const Z_UNKNOWN  :uint = 2;
/* Possible values of the data_type field (though see inflate()) */

pub const Z_DEFLATED :uint = 8;
/* The deflate compression method (the only one supported in this version) */


impl ZStream
{
    pub fn new() -> ZStream
    {
        ZStream {
            next_in: 0,
            avail_in: 0,
            total_in: 0,
            next_out: 0,
            avail_out: 0,
            total_out: 0,
            msg: None,
            data_type: 0,  /* best guess about the data type: binary or text */
            adler: 0,      /* adler32 value of the uncompressed data */
        }
    }
}


/* Maximum value for windowBits in deflateInit2 and inflateInit2.
 * WARNING: reducing MAX_WBITS makes minigzip unable to extract .gz files
 * created by gzip. (Files created by minigzip can still be extracted by
 * gzip.)
 */
pub const MAX_WBITS :uint = 15; /* 32K LZ77 window */
pub const DEF_WBITS :uint = MAX_WBITS;

pub fn swap32(n: u32) -> u32
{
    (n >> 24)
    | ((n >> 8) & 0xff00)
    | ((n << 8) & 0xff0000)
    | (n << 24)
}
