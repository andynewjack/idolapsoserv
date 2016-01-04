//! Shipgate messages and responses.

use std::io;
use std::io::{Read, Write};
use std::net::{SocketAddrV4, Ipv4Addr};

use psoserial::Serial;
use psomsg_common::util::*;

use byteorder::{BigEndian as BE, ReadBytesExt, WriteBytesExt};

macro_rules! impl_shipgate_message_enum {
    ($($id:expr => $name:ident),*) => {
        #[derive(Clone, Debug)]
        pub enum Message {
            Unknown(u16, u32, Vec<u8>),
            $($name(u32, $name)),*
        }

        impl Serial for Message {
            fn serialize(&self, dst: &mut Write) -> io::Result<()> {
                use std::io::Cursor;
                let mut buf = Vec::with_capacity(2048);
                let size;
                let msg_type: u16;
                let response_key: u32;
                debug!("Serializing shipgate message");
                {
                    let mut cur = Cursor::new(buf);
                    match self {
                        &Message::Unknown(ty, res, ref bytes) => {
                            try!(cur.write_all(&bytes[..]));
                            msg_type = ty;
                            response_key = res;
                        },
                        $(&Message::$name(res, ref a) => {
                            try!(a.serialize(&mut cur));
                            msg_type = $id;
                            response_key = res;
                        }),*
                    }
                    size = cur.position() as u16;
                    buf = cur.into_inner();
                }
                debug!("Serializing shipgate header");
                let hdr_buf;
                {
                    let mut curs = Cursor::new(Vec::with_capacity(8));
                    try!(curs.write_u16::<BE>(size + 8));
                    try!(curs.write_u16::<BE>(msg_type));
                    try!(curs.write_u32::<BE>(response_key));
                    hdr_buf = curs.into_inner();
                }
                try!(dst.write_all(&hdr_buf));
                try!(dst.write_all(&buf));
                Ok(())
            }

            fn deserialize(src: &mut Read) -> io::Result<Self> {
                use std::io::Cursor;
                let mut hdr_buf = vec![0; 8];
                debug!("Reading shipgate message header");
                try!(read_exact(src, &mut hdr_buf[..]));
                let size;
                let msg_type;
                let response_key;
                {
                    let mut hdr_cursor = Cursor::new(hdr_buf);
                    size = try!(hdr_cursor.read_u16::<BE>());
                    msg_type = try!(hdr_cursor.read_u16::<BE>());
                    response_key = try!(hdr_cursor.read_u32::<BE>());
                }
                debug!("Shipgate header: size: {}, type: {}, response_key: {}", size, msg_type, response_key);

                let mut msg_buf = vec![0; size as usize - 8];
                try!(read_exact(src, &mut msg_buf));
                let mut msg_cur = Cursor::new(msg_buf);
                let res = match msg_type {
                    $($id => Ok(Message::$name(response_key, try!($name::deserialize(&mut msg_cur)))),)*
                    a => {
                        Ok(Message::Unknown(a, response_key, msg_cur.into_inner()))
                    }
                };
                debug!("Shipgate message parsed: {:?}", res);
                res
            }
        }

        impl Message {
            pub fn set_response_key(&mut self, res_key: u32) {
                match self {
                    $(&mut Message::$name(ref mut res, _) => {
                        *res = res_key;
                    },)*
                    &mut Message::Unknown(_, ref mut res, _) => {
                        *res = res_key;
                    }
                }
            }

            pub fn get_response_key(&self) -> u32 {
                match self {
                    $(&Message::$name(res, _) => {
                        res
                    },)*
                    &Message::Unknown(_, res, _) => {
                        res
                    }
                }
            }
        }

        $(
            impl From<(u32, $name)> for Message {
                #[inline(always)]
                fn from(val: (u32, $name)) -> Message {
                    Message::$name(val.0, val.1)
                }
            }

            impl From<$name> for Message {
                #[inline(always)]
                fn from(val: $name) -> Message {
                    Message::$name(0, val)
                }
            }
        )*

        impl From<(u16, u32, Vec<u8>)> for Message {
            #[inline(always)]
            fn from(val: (u16, u32, Vec<u8>)) -> Message {
                Message::Unknown(val.0, val.1, val.2)
            }
        }
    }
}

impl_shipgate_message_enum! {
    0 => Auth,
    1 => AuthAck,
    2 => BbLoginChallenge,
    3 => BbLoginChallengeAck,
    4 => BbGetAccountInfo,
    5 => BbGetAccountInfoAck,
    6 => RegisterShip,
    7 => RegisterShipAck,
    8 => ShipList,
    9 => ShipListAck
}

#[derive(Clone, Debug)]
pub struct Auth(pub u32, pub String);
impl Serial for Auth {
    fn serialize(&self, dst: &mut Write) -> io::Result<()> {
        try!(self.0.serialize(dst));
        try!(write_utf16(&self.1, dst));
        Ok(())
    }

    fn deserialize(src: &mut Read) -> io::Result<Self> {
        Ok(Auth(try!(Serial::deserialize(src)), try!(read_utf16(src))))
    }
}

derive_serial!(AuthAck);

#[derive(Clone, Debug)]
pub struct BbLoginChallenge {
    pub username: String,
    pub password: String
}
impl Serial for BbLoginChallenge {
    fn serialize(&self, dst: &mut Write) -> io::Result<()> {
        try!(write_utf16(&self.username, dst));
        try!(write_utf16(&self.password, dst));
        Ok(())
    }

    fn deserialize(src: &mut Read) -> io::Result<Self> {
        let username = try!(read_utf16(src));
        let password = try!(read_utf16(src));
        Ok(BbLoginChallenge {
            username: username,
            password: password
        })
    }
}

derive_serial! {
    BbLoginChallengeAck {
        pub status: u32,
        pub account_id: u32
    }
}

derive_serial! {
    BbGetAccountInfo {
        pub account_id: u32
    }
}

derive_serial! {
    BbGetAccountInfoAck {
        pub status: u32,
        pub account_id: u32,
        pub guildcard_num: u32,
        pub team_id: u32
    }
}

#[derive(Clone, Debug)]
pub struct RegisterShip(pub SocketAddrV4, pub String);
impl Serial for RegisterShip {
    fn serialize(&self, dst: &mut Write) -> io::Result<()> {
        let ip = self.0.ip().octets();
        let port = self.0.port();
        try!(write_array(&ip, 4, dst));
        try!(port.serialize(dst));
        try!(write_utf16(&self.1, dst));
        Ok(())
    }

    fn deserialize(src: &mut Read) -> io::Result<Self> {
        let ip_octets = try!(read_array(4, src));
        let port = try!(u16::deserialize(src));
        let socketaddr = SocketAddrV4::new(Ipv4Addr::new(ip_octets[0], ip_octets[1], ip_octets[2], ip_octets[3]), port);
        let name = try!(read_utf16(src));
        Ok(RegisterShip(socketaddr, name))
    }
}

derive_serial!(RegisterShipAck);

derive_serial!(ShipList);

#[derive(Clone, Debug)]
pub struct ShipListAck(pub Vec<(SocketAddrV4, String)>);
impl Serial for ShipListAck {
    fn serialize(&self, dst: &mut Write) -> io::Result<()> {
        try!((self.0.len() as u32).serialize(dst));
        for &(ref s, ref n) in self.0.iter() {
            let ip = s.ip().octets();
            let port = s.port();
            try!(write_array(&ip, 4, dst));
            try!(port.serialize(dst));
            try!(write_utf16(n, dst));
        }
        Ok(())
    }

    fn deserialize(src: &mut Read) -> io::Result<Self> {
        let len = try!(u32::deserialize(src));
        let mut ships = Vec::with_capacity(len as usize);
        for _ in 0..len {
            let ip_octets = try!(read_array(4, src));
            let port = try!(u16::deserialize(src));
            let socketaddr = SocketAddrV4::new(Ipv4Addr::new(ip_octets[0], ip_octets[1], ip_octets[2], ip_octets[3]), port);
            let name = try!(read_utf16(src));
            ships.push((socketaddr, name));
        }
        Ok(ShipListAck(ships))
    }
}
