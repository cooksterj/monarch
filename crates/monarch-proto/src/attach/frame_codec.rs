//! Length-prefixed postcard frames, for both blocking and tokio streams.
//!
//! A frame is a big-endian `u32` body length followed by a postcard-encoded
//! message. The same codec serves the tokio server (`Decoder`/`Encoder`)
//! and the client's blocking threads ([`FrameCodec::encode_to`],
//! [`FrameCodec::decode_from`]).

use std::marker::PhantomData;

use bytes::{Buf, BytesMut};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio_util::codec::{Decoder, Encoder};

use crate::ProtoError;

const HEADER: usize = 4;

/// Codec that decodes `Rx` messages and encodes `Tx` messages.
#[derive(Debug)]
pub struct FrameCodec<Rx, Tx> {
    max_frame: usize,
    _messages: PhantomData<fn(Tx) -> Rx>,
}

impl<Rx, Tx> Default for FrameCodec<Rx, Tx> {
    fn default() -> Self {
        Self::new(Self::MAX_FRAME)
    }
}

impl<Rx, Tx> Clone for FrameCodec<Rx, Tx> {
    fn clone(&self) -> Self {
        Self::new(self.max_frame)
    }
}

impl<Rx, Tx> FrameCodec<Rx, Tx> {
    /// Largest frame accepted by default (16 MiB).
    pub const MAX_FRAME: usize = 16 << 20;

    /// A codec rejecting frames over `max_frame` bytes.
    #[must_use]
    pub fn new(max_frame: usize) -> Self {
        Self {
            max_frame,
            _messages: PhantomData,
        }
    }
}

impl<Rx: DeserializeOwned, Tx: Serialize> FrameCodec<Rx, Tx> {
    /// Appends one framed message to `dst`.
    ///
    /// # Errors
    /// The message cannot be serialized or exceeds the frame limit.
    pub fn encode_to(&self, msg: &Tx, dst: &mut Vec<u8>) -> Result<(), ProtoError> {
        let body = postcard::to_stdvec(msg).map_err(ProtoError::Encode)?;
        let len = self.check_len(body.len())?;
        dst.extend_from_slice(&len.to_be_bytes());
        dst.extend_from_slice(&body);
        Ok(())
    }

    /// Takes one complete message off the front of `src`, if there is one.
    ///
    /// # Errors
    /// The frame is over the limit or its body is not a valid message.
    pub fn decode_from(&self, src: &mut BytesMut) -> Result<Option<Rx>, ProtoError> {
        let Some(header) = src.get(..HEADER) else {
            return Ok(None);
        };
        let len = u32::from_be_bytes(header.try_into().expect("invariant: 4-byte slice"));
        let len = usize::try_from(len).unwrap_or(usize::MAX);
        self.check_len(len)?;
        if src.len() < HEADER + len {
            src.reserve(HEADER + len - src.len());
            return Ok(None);
        }
        src.advance(HEADER);
        let body = src.split_to(len);
        postcard::from_bytes(&body)
            .map(Some)
            .map_err(ProtoError::Decode)
    }

    fn check_len(&self, len: usize) -> Result<u32, ProtoError> {
        let too_large = ProtoError::FrameTooLarge {
            len,
            max: self.max_frame,
        };
        if len > self.max_frame {
            return Err(too_large);
        }
        u32::try_from(len).map_err(|_| too_large)
    }
}

impl<Rx: DeserializeOwned, Tx: Serialize> Decoder for FrameCodec<Rx, Tx> {
    type Item = Rx;
    type Error = ProtoError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Rx>, ProtoError> {
        self.decode_from(src)
    }
}

impl<Rx: DeserializeOwned, Tx: Serialize> Encoder<Tx> for FrameCodec<Rx, Tx> {
    type Error = ProtoError;

    fn encode(&mut self, msg: Tx, dst: &mut BytesMut) -> Result<(), ProtoError> {
        let mut frame = Vec::new();
        self.encode_to(&msg, &mut frame)?;
        dst.extend_from_slice(&frame);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
