//! Tokio codec for framed protocol messages

use bytes::BytesMut;
use tokio_util::codec::{Decoder, Encoder};

use crate::error::ProtocolError;
use crate::frame::{FrameHeader, MAX_PAYLOAD_SIZE};
use crate::message::Message;
use crate::session::SessionId;

/// A complete frame with header and payload
#[derive(Debug, Clone)]
pub struct Frame {
    /// Session ID this frame belongs to
    pub session_id: SessionId,
    /// The message payload
    pub message: Message,
}

impl Frame {
    /// Create a new frame
    pub fn new(session_id: SessionId, message: Message) -> Self {
        Self {
            session_id,
            message,
        }
    }
}

/// Codec for encoding/decoding protocol frames
#[derive(Debug, Default)]
pub struct FrameCodec {
    /// Current header being decoded (if any)
    pending_header: Option<FrameHeader>,
}

impl FrameCodec {
    /// Create a new codec
    pub fn new() -> Self {
        Self {
            pending_header: None,
        }
    }
}

impl Decoder for FrameCodec {
    type Item = Frame;
    type Error = ProtocolError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // Try to decode header if we don't have one
        let header = match self.pending_header.take() {
            Some(h) => h,
            None => match FrameHeader::decode(src)? {
                Some(h) => h,
                None => return Ok(None), // Need more data
            },
        };

        // Check payload length
        let payload_len = header.payload_length as usize;
        if payload_len > MAX_PAYLOAD_SIZE {
            return Err(ProtocolError::PayloadTooLarge {
                size: payload_len,
                max: MAX_PAYLOAD_SIZE,
            });
        }

        // Check if we have enough data for the payload
        if src.len() < payload_len {
            // Save header and wait for more data
            self.pending_header = Some(header);
            return Ok(None);
        }

        // Extract payload
        let payload_bytes = src.split_to(payload_len).freeze();

        // Deserialize message
        let message: Message = bincode::deserialize(&payload_bytes)?;

        Ok(Some(Frame {
            session_id: header.session_id,
            message,
        }))
    }
}

impl Encoder<Frame> for FrameCodec {
    type Error = ProtocolError;

    fn encode(&mut self, frame: Frame, dst: &mut BytesMut) -> Result<(), Self::Error> {
        // Serialize the message
        let payload = bincode::serialize(&frame.message)?;
        let payload_len = payload.len();

        // Check payload size
        if payload_len > MAX_PAYLOAD_SIZE {
            return Err(ProtocolError::PayloadTooLarge {
                size: payload_len,
                max: MAX_PAYLOAD_SIZE,
            });
        }

        // Encode header
        let header = FrameHeader::new(
            frame.session_id,
            frame.message.message_type(),
            payload_len as u32,
        );
        header.encode(dst);

        // Append payload
        dst.extend_from_slice(&payload);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::HEADER_SIZE;
    use crate::message::TerminalSize;
    use bytes::Bytes;

    #[test]
    fn test_codec_roundtrip() {
        let mut codec = FrameCodec::new();

        let frame = Frame::new(
            SessionId::new(1),
            Message::SessionCreate {
                shell: Some("/bin/bash".to_string()),
                env: vec![("TERM".to_string(), "xterm-256color".to_string())],
                initial_size: TerminalSize::new(24, 80),
            },
        );

        // Encode
        let mut buf = BytesMut::new();
        codec.encode(frame.clone(), &mut buf).unwrap();

        // Decode
        let decoded = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(decoded.session_id, frame.session_id);
        // Message comparison would need PartialEq
    }

    #[test]
    fn test_codec_data_message() {
        let mut codec = FrameCodec::new();

        let frame = Frame::new(
            SessionId::new(42),
            Message::Data(Bytes::from("Hello, world!")),
        );

        let mut buf = BytesMut::new();
        codec.encode(frame, &mut buf).unwrap();

        let decoded = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(decoded.session_id, SessionId::new(42));

        if let Message::Data(data) = decoded.message {
            assert_eq!(data.as_ref(), b"Hello, world!");
        } else {
            panic!("Expected Data message");
        }
    }

    #[test]
    fn test_codec_partial_read() {
        let mut codec = FrameCodec::new();

        let frame = Frame::new(SessionId::new(1), Message::Heartbeat { timestamp: 12345 });

        let mut full_buf = BytesMut::new();
        codec.encode(frame, &mut full_buf).unwrap();

        // Split the buffer to simulate partial read
        let mut partial = full_buf.split_to(HEADER_SIZE - 1);

        // Should return None (need more data)
        assert!(codec.decode(&mut partial).unwrap().is_none());

        // Add the rest
        partial.extend_from_slice(&full_buf);

        // Now it should decode
        let decoded = codec.decode(&mut partial).unwrap().unwrap();
        if let Message::Heartbeat { timestamp } = decoded.message {
            assert_eq!(timestamp, 12345);
        } else {
            panic!("Expected Heartbeat message");
        }
    }
}
