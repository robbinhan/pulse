//! Pure feed logic. The TSX screen calls it through [`super::bridge`].

pub mod embed;
pub mod extract;
pub mod ig;
pub mod weibo;
pub mod media;
pub mod rss;
pub mod model;
pub mod notice;
pub mod refresh;
pub mod schedule;
pub mod scroll;
pub mod session;
pub mod sync;
pub mod source;
pub mod briefing;
pub mod text;
pub mod time;
pub mod translate;
