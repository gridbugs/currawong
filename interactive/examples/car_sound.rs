use clap::{Parser, Subcommand};
use currawong_interactive::prelude::*;

struct Effects {
    high_pass_filter_cutoff: Sf64,
    low_pass_filter_cutoff: Sf64,
    low_pass_filter_resonance: Sf64,
    lfo_freq: Sf64,
    lfo_scale: Sf64,
    detune: Sf64,
    echo_scale: Sf64,
}

impl Effects {
    fn new(modulation: Sf64, extra_controllers: &MidiControllerTable) -> Self {
        Self {
            high_pass_filter_cutoff: extra_controllers.get_01(35).exp_01(1.0),
            low_pass_filter_cutoff: extra_controllers.get_01(31).inv_01().exp_01(1.0),
            low_pass_filter_resonance: extra_controllers.get_01(32).exp_01(1.0),
            lfo_freq: extra_controllers.get_01(33),
            lfo_scale: extra_controllers.get_01(34),
            detune: extra_controllers.get_01(36),
            echo_scale: modulation,
        }
    }
}

fn make_voice(
    MidiVoice {
        note_freq,
        velocity_01,
        gate,
        ..
    }: MidiVoice,
    pitch_bend_multiplier_hz: impl Into<Sf64>,
    controllers: &MidiControllerTable,
    serial_controllers: &MidiControllerTable,
) -> Sf64 {
    let Effects {
        high_pass_filter_cutoff,
        low_pass_filter_cutoff,
        low_pass_filter_resonance,
        lfo_freq,
        lfo_scale,
        detune,
        echo_scale,
    } = Effects::new(controllers.modulation(), serial_controllers);
    let velocity_01 = velocity_01.filter(low_pass_butterworth(10.0).build());
    let note_freq_hz = sfreq_to_hz(note_freq)
        .filter(low_pass_butterworth(0.2 + detune * 4.0).build())
        * pitch_bend_multiplier_hz.into()
        * 2;
    let lfo2 = oscillator_hz(Waveform::Sine, (&note_freq_hz / semitone_ratio(19.0)) / 4.0)
        .build()
        .signed_to_01()
        * 0.4
        + 0.5;
    let osc = mean([
        oscillator_hz(Waveform::Saw, &note_freq_hz).build(),
        oscillator_hz(Waveform::Saw, &note_freq_hz * (1.0 + 0.04)).build(),
        oscillator_hz(Waveform::Pulse, &note_freq_hz / semitone_ratio(19.0))
            .pulse_width_01(lfo2)
            .build()
            * 2,
    ]);
    let env = adsr_linear_01(&gate)
        .attack_s(0.001)
        .decay_s(0.5)
        .sustain_01(0.8)
        .release_s(0.0)
        .build()
        .exp_01(1.0);
    let trig = periodic_gate_hz((&note_freq_hz * lfo_freq) * noise().signed_to_01() + 0)
        .build()
        .to_trigger_rising_edge();
    let lfo = noise()
        .filter(low_pass_butterworth(5.0).build())
        .filter(sample_and_hold(trig).build())
        .signed_to_01()
        * lfo_scale;
    let filtered_osc = osc
        .filter(
            low_pass_moog_ladder(sum([
                200.0.into(),
                &env * (low_pass_filter_cutoff * 10_000.0),
                lfo * 10_000.0,
            ]))
            .resonance(low_pass_filter_resonance * 4.0)
            .build(),
        )
        .filter(high_pass_butterworth(high_pass_filter_cutoff * 400.0 + 10.0).build());
    (filtered_osc.mul_lazy(&env) * velocity_01)
        .filter(echo().time_s(0.05).scale(echo_scale).build())
}

fn run(signal: Sf64) -> anyhow::Result<()> {
    let window = Window::builder()
        .scale(10.0)
        .stable(true)
        .spread(2)
        .line_width(4)
        .background(Rgb24::new(0, 31, 0))
        .foreground(Rgb24::new(0, 255, 0))
        .build();
    window.play(signal * 0.1)
}

#[derive(Parser, Debug)]
#[command(name = "midi_live")]
#[command(about = "Example of controlling the synthesizer live with a midi controller")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    ListPorts,
    Play {
        #[arg(short, long)]
        midi_port: usize,
        #[arg(short, long)]
        serial_port: String,
        #[arg(short, long, default_value_t = 115200)]
        serial_baud: u32,
    },
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let args = Cli::parse();
    let midi_live = MidiLive::new()?;
    match args.command {
        Commands::ListPorts => {
            for (i, name) in midi_live.enumerate_port_names() {
                println!("{}: {}", i, name);
            }
        }
        Commands::Play {
            midi_port,
            serial_port,
            serial_baud,
        } => {
            let MidiPlayerMonophonic {
                voice,
                pitch_bend_multiplier_hz,
                controllers,
                ..
            } = midi_live.into_player_monophonic(midi_port, 0).unwrap();
            let MidiChannel {
                controllers: serial_controllers,
                ..
            } = MidiLiveSerial::new(serial_port, serial_baud)?.into_player_single_channel(0, 0);
            let signal = make_voice(
                voice,
                &pitch_bend_multiplier_hz,
                &controllers,
                &serial_controllers,
            )
            .filter(
                saturate()
                    .scale((serial_controllers.get_01(37) * 9.0) + 1.0)
                    .threshold((serial_controllers.get_01(38).inv_01() * 3.9) + 0.1)
                    .build(),
            );
            run(signal)?;
        }
    }
    Ok(())
}
