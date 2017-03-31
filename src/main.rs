use std::io::Read;
use std::process::exit;

// from sysexits.h
#[allow(dead_code)]
enum SysExits {
    Ok = 0,               /* successful termination */
    Usage = 64,           /* command line usage error */
    DataErr = 65,         /* data format error */
    NoInput = 66,         /* cannot open input */
    NoUser = 67,          /* addressee unknown */
    NoHost = 68,          /* host name unknown */
    Unavailable = 69,     /* service unavailable */
    Software = 70,        /* internal software error */
    OsErr = 71,           /* system error (e.g., can't fork) */
    OsFile = 72,          /* critical OS file missing */
    CantCreat = 73,       /* can't create (user) output file */
    IoErr = 74,           /* input/output error */
    TempFail = 75,        /* temp failure; user is invited to retry */
    Protocol = 76,        /* remote error in protocol */
    NoPerm = 77,          /* permission denied */
    Config = 78,          /* configuration error */
}

fn read_leb128<T: Iterator<Item=u8>>(bytes: &mut T) -> Option<u64>
{
    let mut shift = 0;
    let mut acc = 0_u64;
    loop {
        let b = match bytes.next() { None => return None, Some(b) => b };
        acc |= ((b & 0x7f) as u64) << shift;
        shift += 7;
        if 0 == b & 0x80 { return Some(acc) }
    }
}

macro_rules! they_fucked_up {
    ($msg:expr) => ({println!("corrupt: {}", $msg);
                     exit(SysExits::DataErr as i32)});
    ($fmt:expr, $($arg:tt)*) => ({
        println!(concat!("corrupt: ", $fmt), $($arg)*);
        exit(SysExits::DataErr as i32)});
}

fn dump_varint<T: Iterator<Item=u8>>(bytes: &mut T)
{
    let x = match read_leb128(bytes) {
        None => they_fucked_up!("bad varint"),
        Some(x) => x
    };
    println!("varint {}", x)
}

fn dump_fixed32<T: Iterator<Item=u8>>(bytes: &mut T)
{
    println!("fixed32 {}",
             bytes.take(4).fold(0_u32, |acc, b| (acc<<8) | (b as u32)));
}

fn dump_fixed64<T: Iterator<Item=u8>>(bytes: &mut T)
{
    println!("fixed64 {}",
             bytes.take(8).fold(0_u64, |acc, b| (acc<<8) | (b as u64)));
}

fn all_printable(s: &Vec<u8>) -> bool {
    for &b in s.iter() { if b < 0x20 || b > 0x7f { return false } }
    true
}

fn dump_string<T: Iterator<Item=u8>>(bytes: &mut T)
{
    let len = match read_leb128(bytes) {
        None => they_fucked_up!("bad length on string tag"),
        Some(len) => len
    };
    print!("{}-byte string: ", len);
    let s = bytes.take(len as usize).collect();
    if all_printable(&s) {
        print!("{}", String::from_utf8(s).unwrap())
    } else {
        for b in s.iter() { print!("{:x} ", b) }
    }
    println!()
}

fn decode_one<T: Iterator<Item=u8>>(bytes: &mut T)
{
    let mut max_field_seen = 0;
    loop {
        let b = match bytes.next() { None => return, Some(b) => b };

        let field = b >> 3;
        let tag = b & 7;
        if field < max_field_seen {
            println!("Warning: message has out-of-order fields ({} < {})",
                     field, max_field_seen)
        } else {
            max_field_seen = field
        }

        print!("{}: ", field);
        match tag {
            0 => dump_varint(bytes),
            1 => dump_fixed32(bytes),
            2 => dump_string(bytes),
            3 => {
                println!("should dump a group here, but we don't know \
                          how yet.  sorry.");
                exit(SysExits::Software as i32)
            },
            5 => dump_fixed64(bytes),
            _ => they_fucked_up!("invalid tag {} at field {}", tag, field)
        }
    }
}

#[derive(PartialEq)]
enum Mode { Multiple, Single }

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let mut mode = Mode::Multiple;
    if args.len() == 2 && args[1] == "single" {
        mode = Mode::Single;
    } else if args.len() != 1 {
        println!("This tool is very dumb.  \
                  Pass the argument \"single\" to read a single protobuf \
                  message, otherwise we try to read a stream of (LEB128) \
                  length-delimited messages.  stdin only.");
        exit(SysExits::Usage as i32)
    }

    let mut stdin = std::io::stdin();
    if mode == Mode::Single {
        let mut buf = Vec::new();
        stdin.read_to_end(&mut buf).unwrap();
        decode_one(&mut buf.into_iter());
        exit(SysExits::Ok as i32)
    }

    let mut bytes = stdin.lock().bytes().map(|x| x.unwrap()).peekable();
    loop {
        if None == bytes.peek() { exit(SysExits::Ok as i32) }
        let len = match read_leb128(&mut bytes) {
            None => they_fucked_up!("bad length on message"),
            Some(len) => len
        };
        let mut buf = Vec::new();
        for _ in 0..len { buf.push(bytes.next().unwrap()) }
        decode_one(&mut buf.into_iter())
    }
}
