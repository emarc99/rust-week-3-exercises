use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct CompactSize {
    pub value: u64,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BitcoinError {
    InsufficientBytes,
    InvalidFormat,
}

impl CompactSize {
    pub fn new(value: u64) -> Self {
        Self { value }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        match self.value {
            0..=252 => vec![self.value as u8],
            253..=0xffff => {
                let mut v = vec![0xfd];
                v.extend_from_slice(&(self.value as u16).to_le_bytes());
                v
            }
            0x10000..=0xffff_ffff => {
                let mut v = vec![0xfe];
                v.extend_from_slice(&(self.value as u32).to_le_bytes());
                v
            }
            _ => {
                let mut v = vec![0xff];
                v.extend_from_slice(&(self.value as u64).to_le_bytes());
                v
            }
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.is_empty() {
            return Err(BitcoinError::InsufficientBytes);
        }

        match bytes[0] {
            n @ 0x00..=0xfc => Ok((CompactSize::new(n as u64), 1)),
            0xfd => {
                if bytes.len() < 3 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let val = u16::from_le_bytes([bytes[1], bytes[2]]) as u64;
                Ok((CompactSize::new(val), 3))
            }
            0xfe => {
                if bytes.len() < 5 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let val = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as u64;
                Ok((CompactSize::new(val), 5))
            }
            0xff => {
                if bytes.len() < 9 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let val = u64::from_le_bytes([
                    bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8],
                ]);
                Ok((CompactSize::new(val), 9))
            } // _ => Err(BitcoinError::InvalidFormat),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Txid(pub [u8; 32]);

impl Serialize for Txid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&hex::encode(&self.0))
    }
}

impl<'de> Deserialize<'de> for Txid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let hex_str = String::deserialize(deserializer)?;
        let bytes = hex::decode(&hex_str).map_err(serde::de::Error::custom)?;
        if bytes.len() != 32 {
            return Err(serde::de::Error::custom("Txid must be 32 bytes"));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Txid(arr))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct OutPoint {
    pub txid: Txid,
    pub vout: u32,
}

impl OutPoint {
    pub fn new(txid: [u8; 32], vout: u32) -> Self {
        Self {
            txid: Txid(txid),
            vout,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = self.txid.0.to_vec();
        v.extend_from_slice(&self.vout.to_le_bytes());
        v
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.len() < 36 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let mut txid = [0u8; 32];
        txid.copy_from_slice(&bytes[..32]);
        let vout = u32::from_le_bytes([bytes[32], bytes[33], bytes[34], bytes[35]]);
        Ok((OutPoint::new(txid, vout), 36))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Script {
    pub bytes: Vec<u8>,
}

impl Script {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = CompactSize::new(self.bytes.len() as u64).to_bytes();
        v.extend_from_slice(&self.bytes);
        v
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let (len_prefix, consumed) = CompactSize::from_bytes(bytes)?;
        let len = len_prefix.value as usize;
        if bytes.len() < consumed + len {
            return Err(BitcoinError::InsufficientBytes);
        }
        let data = bytes[consumed..consumed + len].to_vec();
        Ok((Script::new(data), consumed + len))
    }
}

impl Deref for Script {
    type Target = Vec<u8>;
    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct TransactionInput {
    pub previous_output: OutPoint,
    pub script_sig: Script,
    pub sequence: u32,
}

impl TransactionInput {
    pub fn new(previous_output: OutPoint, script_sig: Script, sequence: u32) -> Self {
        Self {
            previous_output,
            script_sig,
            sequence,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = self.previous_output.to_bytes();
        v.extend(self.script_sig.to_bytes());
        v.extend_from_slice(&self.sequence.to_le_bytes());
        v
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        // TODO: Deserialize in order:
        let (outpoint, used1) = OutPoint::from_bytes(bytes)?;
        let (script, used2) = Script::from_bytes(&bytes[used1..])?;
        if bytes.len() < used1 + used2 + 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let sequence = u32::from_le_bytes([
            bytes[used1 + used2],
            bytes[used1 + used2 + 1],
            bytes[used1 + used2 + 2],
            bytes[used1 + used2 + 3],
        ]);
        Ok((
            TransactionInput::new(outpoint, script, sequence),
            used1 + used2 + 4,
        ))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct BitcoinTransaction {
    pub version: u32,
    pub inputs: Vec<TransactionInput>,
    pub lock_time: u32,
}

impl BitcoinTransaction {
    pub fn new(version: u32, inputs: Vec<TransactionInput>, lock_time: u32) -> Self {
        Self {
            version,
            inputs,
            lock_time,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = self.version.to_le_bytes().to_vec();
        v.extend(CompactSize::new(self.inputs.len() as u64).to_bytes());
        for input in &self.inputs {
            v.extend(input.to_bytes());
        }
        v.extend(&self.lock_time.to_le_bytes());
        v
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.len() < 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let version = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let (input_count, offset) = CompactSize::from_bytes(&bytes[4..])?;
        let mut inputs = Vec::new();
        let mut cursor = 4 + offset;
        for _ in 0..input_count.value {
            let (input, used) = TransactionInput::from_bytes(&bytes[cursor..])?;
            inputs.push(input);
            cursor += used;
        }
        if bytes.len() < cursor + 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let lock_time = u32::from_le_bytes([
            bytes[cursor],
            bytes[cursor + 1],
            bytes[cursor + 2],
            bytes[cursor + 3],
        ]);
        Ok((
            BitcoinTransaction::new(version, inputs, lock_time),
            cursor + 4,
        ))
    }
}

impl fmt::Display for BitcoinTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Version: {}", self.version)?;
        for input in &self.inputs {
            writeln!(f, "Previous Output Vout: {}", input.previous_output.vout)?;
            writeln!(f, "ScriptSig: {} bytes", input.script_sig.bytes.len())?;
        }
        writeln!(f, "Lock Time: {}", self.lock_time)
    }
}
