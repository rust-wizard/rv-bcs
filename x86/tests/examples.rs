use bcs::{from_bytes, to_bytes, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[test]
fn optional_data_example() -> Result<()> {
    let some_data: Option<u8> = Some(8);
    assert_eq!(to_bytes(&some_data)?, vec![1, 8]);

    let no_data: Option<u8> = None;
    assert_eq!(to_bytes(&no_data)?, vec![0]);
    Ok(())
}

#[test]
fn fixed_and_variable_length_sequences_example() -> Result<()> {
    let fixed: [u16; 3] = [1, 2, 3];
    assert_eq!(to_bytes(&fixed)?, vec![1, 0, 2, 0, 3, 0]);

    let variable: Vec<u16> = vec![1, 2];
    assert_eq!(to_bytes(&variable)?, vec![2, 1, 0, 2, 0]);

    let large_variable_length: Vec<()> = vec![(); 9_487];
    assert_eq!(to_bytes(&large_variable_length)?, vec![0x8f, 0x4a]);
    Ok(())
}

#[test]
fn strings_example() -> Result<()> {
    let utf8_str = "çå∞≠¢õß∂ƒ∫";
    let expecting = vec![
        24, 0xc3, 0xa7, 0xc3, 0xa5, 0xe2, 0x88, 0x9e, 0xe2, 0x89, 0xa0, 0xc2, 0xa2, 0xc3, 0xb5,
        0xc3, 0x9f, 0xe2, 0x88, 0x82, 0xc6, 0x92, 0xe2, 0x88, 0xab,
    ];
    assert_eq!(to_bytes(&utf8_str)?, expecting);
    Ok(())
}

#[test]
fn tuples_example() -> Result<()> {
    let tuple = (-1i8, "diem");
    let expecting = vec![0xFF, 4, b'd', b'i', b'e', b'm'];
    assert_eq!(to_bytes(&tuple)?, expecting);
    Ok(())
}

#[derive(Serialize)]
struct MyStruct {
    boolean: bool,
    bytes: Vec<u8>,
    label: String,
}

#[derive(Serialize)]
struct Wrapper {
    inner: MyStruct,
    name: String,
}

#[test]
fn struct_example() -> Result<()> {
    let s = MyStruct {
        boolean: true,
        bytes: vec![0xC0, 0xDE],
        label: "a".to_owned(),
    };
    let s_bytes = to_bytes(&s)?;
    let mut expecting = vec![1, 2, 0xC0, 0xDE, 1, b'a'];
    assert_eq!(s_bytes, expecting);

    let w = Wrapper {
        inner: s,
        name: "b".to_owned(),
    };
    let w_bytes = to_bytes(&w)?;
    assert!(w_bytes.starts_with(&s_bytes));

    expecting.append(&mut vec![1, b'b']);
    assert_eq!(w_bytes, expecting);
    Ok(())
}

#[derive(Serialize)]
enum E {
    Variant0(u16),
    Variant1(u8),
    Variant2(String),
}

#[test]
fn enum_example() -> Result<()> {
    let v0 = E::Variant0(8000);
    let v1 = E::Variant1(255);
    let v2 = E::Variant2("e".to_owned());

    assert_eq!(to_bytes(&v0)?, vec![0, 0x40, 0x1F]);
    assert_eq!(to_bytes(&v1)?, vec![1, 0xFF]);
    assert_eq!(to_bytes(&v2)?, vec![2, 1, b'e']);
    Ok(())
}

#[test]
fn map_example() -> Result<()> {
    let mut map = HashMap::new();
    map.insert(b'e', b'f');
    map.insert(b'a', b'b');
    map.insert(b'c', b'd');

    let expecting = vec![(b'a', b'b'), (b'c', b'd'), (b'e', b'f')];
    assert_eq!(to_bytes(&map)?, to_bytes(&expecting)?);
    Ok(())
}

#[derive(Serialize)]
struct Ip([u8; 4]);

#[derive(Serialize, Deserialize)]
struct Port(u16);

#[derive(Serialize)]
struct Service {
    ip: Ip,
    port: Vec<Port>,
    connection_max: Option<u32>,
    enabled: bool,
}

#[test]
fn serializer_example() {
    let service = Service {
        ip: Ip([192, 168, 1, 1]),
        port: vec![Port(8001), Port(8002), Port(8003)],
        connection_max: Some(5000),
        enabled: false,
    };

    let bytes = to_bytes(&service).unwrap();
    let expected = vec![
        0xc0, 0xa8, 0x01, 0x01, 0x03, 0x41, 0x1f, 0x42, 0x1f, 0x43, 0x1f, 0x01, 0x88, 0x13, 0x00,
        0x00, 0x00,
    ];
    assert_eq!(bytes, expected);
}

#[derive(Deserialize)]
struct DeIp([u8; 4]);

#[derive(Deserialize)]
struct DePort(u16);

#[derive(Deserialize)]
struct SocketAddr {
    ip: DeIp,
    port: DePort,
}

#[test]
fn deserializer_example() {
    let bytes = vec![0x7f, 0x00, 0x00, 0x01, 0x41, 0x1f];
    let socket_addr: SocketAddr = from_bytes(&bytes).unwrap();

    assert_eq!(socket_addr.ip.0, [127, 0, 0, 1]);
    assert_eq!(socket_addr.port.0, 8001);
}
