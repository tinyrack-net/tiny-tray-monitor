//! 표시 문자열 캐시.
//!
//! 값의 종류가 유한하므로(0~100%, 단위 조합) 문자열 단위로 캐시하면 대부분
//! 재사용된다. 픽셀 폰트는 GDI보다 싸지만, 매 갱신마다 비트맵을 새로 할당하지
//! 않도록 캐시는 유지한다.

use std::collections::HashMap;

use super::pixel::{self, Face};
use super::render::{Bitmap, Color};

/// 캐시 조회 키.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GlyphRequest {
    pub text: String,
    pub face: Face,
    pub color: Color,
    pub alpha: u8,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub entries: usize,
}

impl CacheStats {
    /// 재사용 비율(%). 캐시가 실제로 일하고 있는지 확인하는 지표.
    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64 * 100.0
        }
    }
}

#[derive(Debug)]
pub struct GlyphCache {
    entries: HashMap<GlyphRequest, Bitmap>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl GlyphCache {
    pub const DEFAULT_CAPACITY: usize = 256;

    pub fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            capacity: capacity.max(1),
            hits: 0,
            misses: 0,
        }
    }

    /// 캐시에 있으면 그대로, 없으면 렌더링해서 돌려준다.
    ///
    /// 그릴 글리프가 없으면 `None`이다. 호출부는 그 줄을 비워 둔다.
    pub fn get(&mut self, request: GlyphRequest) -> Option<Bitmap> {
        if let Some(bitmap) = self.entries.get(&request) {
            self.hits += 1;
            return Some(bitmap.clone());
        }

        self.misses += 1;
        let bitmap = pixel::render_line(&request.text, request.face, request.color, request.alpha)?;

        // 작업 집합이 작으므로 가득 차면 통째로 버리는 편이 단순하고 충분하다.
        if self.entries.len() >= self.capacity {
            self.entries.clear();
        }
        self.entries.insert(request, bitmap.clone());
        Some(bitmap)
    }

    pub fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits,
            misses: self.misses,
            entries: self.entries.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(text: &str) -> GlyphRequest {
        GlyphRequest {
            text: text.to_string(),
            face: Face::Value,
            color: Color(255, 255, 255),
            alpha: 255,
        }
    }

    #[test]
    fn second_lookup_is_a_hit() {
        let mut cache = GlyphCache::new(16);
        assert!(cache.get(request("80%")).is_some());
        assert!(cache.get(request("80%")).is_some());
        let stats = cache.stats();
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.entries, 1);
        assert!((stats.hit_ratio() - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn different_texts_are_different_entries() {
        let mut cache = GlyphCache::new(16);
        let _ = cache.get(request("80%"));
        let _ = cache.get(request("77%"));
        assert_eq!(cache.stats().entries, 2);
    }

    /// 같은 글자라도 face가 다르면 다른 글리프다.
    #[test]
    fn faces_do_not_share_entries() {
        let mut cache = GlyphCache::new(16);
        let _ = cache.get(request("M"));
        let _ = cache.get(GlyphRequest {
            face: Face::Label,
            ..request("M")
        });
        assert_eq!(cache.stats().entries, 2);
    }

    #[test]
    fn empty_text_is_not_cached() {
        let mut cache = GlyphCache::new(16);
        assert!(cache.get(request("")).is_none());
        assert_eq!(cache.stats().entries, 0);
    }

    #[test]
    fn cache_clears_instead_of_growing_past_capacity() {
        let mut cache = GlyphCache::new(4);
        for i in 0..10 {
            let _ = cache.get(request(&i.to_string()));
        }
        assert!(cache.stats().entries <= 4);
    }

    #[test]
    fn hit_ratio_of_unused_cache_is_zero() {
        assert_eq!(GlyphCache::new(4).stats().hit_ratio(), 0.0);
    }
}
