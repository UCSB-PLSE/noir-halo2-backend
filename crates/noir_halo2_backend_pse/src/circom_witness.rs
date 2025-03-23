//! Implementation of binary .wtns file parser/serializer.
//! According to https://github.com/iden3/snarkjs/blob/master/src/wtns_utils.js

use std::{
    collections::BTreeMap,
    fs::File,
    io::{Error, ErrorKind, Read, Result, Write},
};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use acvm::acir::{
    native_types::{Witness, WitnessMap},
    FieldElement,
};

const MAGIC: &[u8; 4] = b"wtns";

#[derive(Debug, PartialEq)]
pub struct WtnsFile<const FS: usize> {
    pub version: u32,
    pub header: Header<FS>,
    pub witness: CircomWitness<FS>,
}

impl<const FS: usize> WtnsFile<FS> {
    pub fn from_vec(witness: Vec<FB<FS>>, prime: FB<FS>) -> Self {
        WtnsFile {
            version: 1,
            header: Header { field_size: FS as u32, prime, witness_len: witness.len() as u32 },
            witness: CircomWitness(witness),
        }
    }

    pub fn read<R: Read>(mut r: R) -> Result<Self> {
        let mut magic = [0u8; 4];
        r.read_exact(&mut magic)?;

        if magic != *MAGIC {
            return Err(Error::new(ErrorKind::InvalidData, "Invalid magic number"));
        }

        let version = r.read_u32::<LittleEndian>()?;
        if version > 2 {
            return Err(Error::new(ErrorKind::InvalidData, "Unsupported version"));
        }

        let num_sections = r.read_u32::<LittleEndian>()?;
        if num_sections > 2 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Number of sections >2 is not supported",
            ));
        }

        let header = Header::read(&mut r)?;
        let witness = CircomWitness::read(&mut r, &header)?;

        Ok(WtnsFile { version, header, witness })
    }

    pub fn write<W: Write>(&self, mut w: W) -> Result<()> {
        w.write_all(MAGIC)?;
        w.write_u32::<LittleEndian>(self.version)?;
        w.write_u32::<LittleEndian>(2)?;
        self.header.write(&mut w)?;
        self.witness.write(&mut w)?;

        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub struct Header<const FS: usize> {
    pub field_size: u32,
    pub prime: FB<FS>,
    pub witness_len: u32,
}

impl<const FS: usize> Header<FS> {
    pub fn read<R: Read>(mut r: R) -> Result<Self> {
        let sec_type = SectionType::read(&mut r)?;
        if sec_type != SectionType::Header {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Invalid section type: expected header",
            ));
        }

        let sec_size = r.read_u64::<LittleEndian>()?;
        if sec_size != 4 + FS as u64 + 4 {
            return Err(Error::new(ErrorKind::InvalidData, "Invalid header section size"));
        }

        let field_size = r.read_u32::<LittleEndian>()?;
        let prime = FB::read(&mut r)?;

        if field_size != FS as u32 {
            return Err(Error::new(ErrorKind::InvalidData, "Wrong field size"));
        }

        let witness_len = r.read_u32::<LittleEndian>()?;

        Ok(Header { field_size, prime, witness_len })
    }

    pub fn write<W: Write>(&self, mut w: W) -> Result<()> {
        SectionType::Header.write(&mut w)?;

        let sec_size = 4 + FS as u64 + 4;
        w.write_u64::<LittleEndian>(sec_size)?;

        w.write_u32::<LittleEndian>(FS as u32)?;
        self.prime.write(&mut w)?;
        w.write_u32::<LittleEndian>(self.witness_len)?;

        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub struct CircomWitness<const FS: usize>(pub Vec<FB<FS>>);

impl<const FS: usize> CircomWitness<FS> {
    pub fn read<R: Read>(mut r: R, header: &Header<FS>) -> Result<Self> {
        let sec_type = SectionType::read(&mut r)?;
        if sec_type != SectionType::CircomWitness {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Invalid section type: expected witness",
            ));
        }
        let sec_size = r.read_u64::<LittleEndian>()?;

        if sec_size != header.witness_len as u64 * FS as u64 {
            return Err(Error::new(ErrorKind::InvalidData, "Invalid witness section size"));
        }

        let mut witness = Vec::with_capacity(header.witness_len as usize);
        for _ in 0..header.witness_len {
            witness.push(FB::read(&mut r)?);
        }

        Ok(CircomWitness(witness))
    }

    fn write<W: Write>(&self, mut w: W) -> Result<()> {
        SectionType::CircomWitness.write(&mut w)?;

        let sec_size = (self.0.len() * FS) as u64;
        w.write_u64::<LittleEndian>(sec_size)?;

        for e in &self.0 {
            e.write(&mut w)?;
        }

        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
#[repr(u32)]
pub enum SectionType {
    Header = 1,
    CircomWitness = 2,
    Unknown = u32::MAX,
}

impl SectionType {
    fn read<R: Read>(mut r: R) -> Result<Self> {
        let num = r.read_u32::<LittleEndian>()?;

        let ty = match num {
            1 => SectionType::Header,
            2 => SectionType::CircomWitness,
            _ => SectionType::Unknown,
        };

        Ok(ty)
    }

    fn write<W: Write>(&self, mut w: W) -> Result<()> {
        w.write_u32::<LittleEndian>(*self as u32)?;

        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct FB<const FS: usize>([u8; FS]);

impl<const FS: usize> FB<FS> {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0[..]
    }

    fn read<R: Read>(mut r: R) -> Result<Self> {
        let mut buf = [0; FS];
        r.read_exact(&mut buf)?;

        Ok(FB(buf))
    }

    fn write<W: Write>(&self, mut w: W) -> Result<()> {
        w.write_all(&self.0[..])
    }
}

impl<const FS: usize> From<[u8; FS]> for FB<FS> {
    fn from(array: [u8; FS]) -> Self {
        FB(array)
    }
}

impl<const FS: usize> std::ops::Deref for FB<FS> {
    type Target = [u8; FS];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub fn read_file(filename: &str) -> WitnessMap {
    println!("Reading file: {}", filename);
    let f = File::open(filename).unwrap();
    const FS: usize = 32;
    let file = WtnsFile::<FS>::read(f).unwrap();
    let ws = file
        .witness
        .0
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != 0)
        .map(|(i, e)| {
            let mut bs = e.as_bytes().to_vec();
            bs.reverse();
            let f = FieldElement::from_be_bytes_reduce(&bs);
            (Witness::from((i - 1) as u32), f)
        })
        .into_iter();
    let map = BTreeMap::from_iter(ws);
    WitnessMap::from(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const FS: usize = 32;

    fn fe() -> FB<FS> {
        FB::from([
            1, 0, 1, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 1,
        ])
    }

    #[test]
    fn test() {
        let file = WtnsFile::<FS>::from_vec(vec![fe(), fe(), fe()], fe());
        let mut data = Vec::new();
        file.write(&mut data).unwrap();

        let new_file = WtnsFile::read(Cursor::new(data)).unwrap();

        assert_eq!(file, new_file);
    }

    #[test]
    fn test_file() {
        let filename = "example/circuit_js/witness.wtns";

        for (i, f) in read_file(filename) {
            println!("{:?}: {:?}", i, f);
        }
    }
}
