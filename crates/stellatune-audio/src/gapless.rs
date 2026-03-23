use stellatune_audio_core::pipeline::context::GaplessTrimSpec;

pub fn gapless_trimmed_duration_ms(
    duration_ms: Option<u64>,
    sample_rate: u32,
    gapless_trim_spec: Option<GaplessTrimSpec>,
) -> Option<u64> {
    let duration_ms = duration_ms?;
    let sample_rate = sample_rate.max(1) as u128;
    let trimmed_frames = gapless_trim_spec.map_or(0_u128, |spec| {
        (spec.head_frames as u128).saturating_add(spec.tail_frames as u128)
    });
    if trimmed_frames == 0 {
        return Some(duration_ms);
    }

    let trimmed_ms = trimmed_frames
        .saturating_mul(1000)
        .saturating_add(sample_rate / 2)
        .saturating_div(sample_rate)
        .min(u64::MAX as u128) as u64;
    Some(duration_ms.saturating_sub(trimmed_ms))
}

#[cfg(test)]
mod tests {
    use stellatune_audio_core::pipeline::context::GaplessTrimSpec;

    use super::gapless_trimmed_duration_ms;

    #[test]
    fn returns_none_when_duration_is_unknown() {
        assert_eq!(gapless_trimmed_duration_ms(None, 44_100, None), None);
    }

    #[test]
    fn keeps_duration_when_gapless_trim_is_disabled() {
        assert_eq!(
            gapless_trimmed_duration_ms(Some(1_234), 44_100, None),
            Some(1_234)
        );
    }

    #[test]
    fn subtracts_rounded_gapless_trim_duration() {
        let spec = GaplessTrimSpec {
            head_frames: 220,
            tail_frames: 221,
        };
        assert_eq!(
            gapless_trimmed_duration_ms(Some(100), 44_100, Some(spec)),
            Some(90)
        );
    }

    #[test]
    fn saturates_at_zero_when_trim_exceeds_duration() {
        let spec = GaplessTrimSpec {
            head_frames: 44_100,
            tail_frames: 44_100,
        };
        assert_eq!(
            gapless_trimmed_duration_ms(Some(500), 44_100, Some(spec)),
            Some(0)
        );
    }
}
