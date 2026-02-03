//! Frame header encoding/decoding
//!
//! The frame format uses an 8-byte header:
//! - session_id: 4 bytes (u32, big-endian)
//! - message_type: 1 byte (u8)
//! - payload_length: 3 bytes (u24, big-endian, max 16MB)

use bytes::{Buf, BufMut, BytesMut};

use crate::error::ProtocolError;
use crate::message::MessageType;
use crate::session::SessionId;

/// Size of the frame header in bytes
pub const HEADER_SIZE: usize = 8;

/// Maximum payload size (16MB - 1, limited by 24-bit length field)
pub const MAX_PAYLOAD_SIZE: usize = 0x00FF_FFFF;

/// Frame header containing routing and length information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    /// Session this frame belongs to
    pub session_id: SessionId,
    /// Type of message in the payload
    pub message_type: MessageType,
    /// Length of the payload in bytes
    pub payload_length: u32,
}

impl FrameHeader {
    /// Create a new frame header
    pub fn new(session_id: SessionId, message_type: MessageType, payload_length: u32) -> Self {
        Self {
            session_id,
            message_type,
            payload_length,
        }
    }

    /// Encode the header into a byte buffer
    pub fn encode(&self, dst: &mut BytesMut) {
        dst.reserve(HEADER_SIZE);
        // session_id: 4 bytes big-endian
        dst.put_u32(self.session_id.as_u32());
        // message_type: 1 byte
        dst.put_u8(self.message_type.as_u8());
        // payload_length: 3 bytes big-endian (24-bit)
        dst.put_u8((self.payload_length >> 16) as u8);
        dst.put_u16(self.payload_length as u16);
    }

    /// Decode a header from a byte buffer
    ///
    /// Returns None if there aren't enough bytes in the buffer.
    /// Returns Err if the header is invalid (unknown message type).
    pub fn decode(src: &mut BytesMut) -> Result<Option<Self>, ProtocolError> {
        if src.len() < HEADER_SIZE {
            return Ok(None);
        }

        // Peek at the message type first to validate
        let msg_type_byte = src[4];
        let message_type = MessageType::from_u8(msg_type_byte)
            .ok_or(ProtocolError::UnknownMessageType(msg_type_byte))?;

        // Now consume the bytes
        let session_id = SessionId::new(src.get_u32());
        let _ = src.get_u8(); // message_type already parsed
        let len_high = src.get_u8() as u32;
        let len_low = src.get_u16() as u32;
        let payload_length = (len_high << 16) | len_low;

        Ok(Some(Self {
            session_id,
            message_type,
            payload_length,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_roundtrip() {
        let header = FrameHeader::new(SessionId::new(42), MessageType::Data, 12345);

        let mut buf = BytesMut::with_capacity(HEADER_SIZE);
        header.encode(&mut buf);

        assert_eq!(buf.len(), HEADER_SIZE);

        let decoded = FrameHeader::decode(&mut buf).unwrap().unwrap();
        assert_eq!(decoded, header);
    }

    #[test]
    fn test_max_payload_length() {
        let header = FrameHeader::new(
            SessionId::new(1),
            MessageType::Data,
            MAX_PAYLOAD_SIZE as u32,
        );

        let mut buf = BytesMut::with_capacity(HEADER_SIZE);
        header.encode(&mut buf);

        let decoded = FrameHeader::decode(&mut buf).unwrap().unwrap();
        assert_eq!(decoded.payload_length, MAX_PAYLOAD_SIZE as u32);
    }

    #[test]
    fn test_insufficient_bytes() {
        let mut buf = BytesMut::from(&[0u8; 4][..]);
        let result = FrameHeader::decode(&mut buf).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_unknown_message_type() {
        let mut buf = BytesMut::from(&[0, 0, 0, 1, 0xFE, 0, 0, 10][..]);
        let result = FrameHeader::decode(&mut buf);
        assert!(matches!(
            result,
            Err(ProtocolError::UnknownMessageType(0xFE))
        ));
    }
}
