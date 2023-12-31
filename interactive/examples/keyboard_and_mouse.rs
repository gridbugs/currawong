use currawong_interactive::prelude::*;

fn freq_hz_by_gate() -> Vec<(Key, f64)> {
    use Key::*;
    let top_row_base = Note::new(NoteName::B, 2).to_midi_index();
    let top_row = vec![
        Q,
        W,
        N3,
        E,
        N4,
        R,
        T,
        N5,
        Y,
        N7,
        U,
        N8,
        I,
        O,
        N0,
        P,
        Minus,
        LeftBracket,
        RightBracket,
    ];
    let bottom_row_base = Note::new(NoteName::B, 1).to_midi_index();
    let bottom_row = vec![Z, X, D, C, F, V, B, H, N, J, M, K, Comma, Period];
    top_row
        .into_iter()
        .enumerate()
        .map(|(i, key)| (key, freq_hz_of_midi_index(i as u8 + top_row_base)))
        .chain(
            bottom_row
                .into_iter()
                .enumerate()
                .map(|(i, key)| (key, freq_hz_of_midi_index(i as u8 + bottom_row_base))),
        )
        .collect::<Vec<_>>()
}

fn single_voice(freq_hz: f64, gate: Gate, effect_x: Sf64, effect_y: Sf64) -> Sf64 {
    let freq_hz = const_(freq_hz);
    let oscillator = oscillator_hz(Waveform::Saw, &freq_hz).build()
        + oscillator_hz(Waveform::Saw, &freq_hz)
            .reset_offset_01(0.5)
            .build();
    let amp_env = adsr_linear_01(&gate).release_s(0.5).build();
    let filter_env = adsr_linear_01(&gate)
        .attack_s(0.0)
        .decay_s(0.1)
        .sustain_01(0.6)
        .release_s(0.5)
        .build();
    oscillator
        .filter(
            low_pass_moog_ladder(&filter_env * 12000.0 * effect_x)
                .resonance(effect_y * 4.0)
                .build(),
        )
        .mul_lazy(&amp_env)
        .force_lazy(&filter_env)
}

fn voice(input: Input) -> Sf64 {
    let dry = freq_hz_by_gate()
        .into_iter()
        .map(|(key, freq_hz)| {
            single_voice(
                freq_hz,
                input.key(key),
                input.x_01().clone(),
                input.y_01().clone(),
            )
        })
        .sum::<Sf64>()
        .filter(compress().ratio(0.1).scale(2.0).build());
    dry.filter(reverb().room_size(1.0).build()) + dry
}

fn main() -> anyhow::Result<()> {
    let window = Window::builder()
        .scale(4.0)
        .stable(true)
        .spread(1)
        .line_width(4)
        .background(Rgb24::new(0, 31, 0))
        .foreground(Rgb24::new(0, 255, 0))
        .build();
    let signal = voice(window.input());
    window.play(signal * 0.1)
}
