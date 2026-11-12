use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, decoder::LoopedDecoder};
use std::{error::Error, fs::File, io::BufReader, path::Path};

pub const MUSIC_VOLUME: f32 = 0.25;
const MUSIC_PATH: &str = "assets/audio/HansZimmer_Interstellar.mp3";

pub struct Music {
    // El reproductor se libera antes que la salida de audio al cerrar el juego.
    _player: Sink,
    _stream: OutputStream,
}

fn load_track(path: &Path) -> Result<LoopedDecoder<BufReader<File>>, Box<dyn Error>> {
    // El archivo se vuelve a leer al terminar, sin almacenar toda la cancion decodificada.
    Ok(Decoder::new_looped(BufReader::new(File::open(path)?))?)
}

impl Music {
    pub fn start() -> Option<Self> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(MUSIC_PATH);
        match Self::open(&path) {
            Ok(music) => Some(music),
            Err(error) => {
                eprintln!("Musica desactivada; el juego continuara sin audio: {error}");
                None
            }
        }
    }

    fn open(path: &Path) -> Result<Self, Box<dyn Error>> {
        let source = load_track(path)?;
        let mut stream = OutputStreamBuilder::open_default_stream()?;
        stream.log_on_drop(false);
        let player = Sink::connect_new(stream.mixer());
        player.set_volume(MUSIC_VOLUME);
        player.append(source);
        Ok(Self {
            _player: player,
            _stream: stream,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supplied_mp3_decodes_and_restarts_after_the_last_sample() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(MUSIC_PATH);
        let samples = Decoder::try_from(File::open(&path).unwrap())
            .unwrap()
            .count();
        assert!(samples > 0);
        let mut track = load_track(&path).unwrap();
        let beginning: Vec<_> = track.by_ref().take(1024).collect();
        assert_eq!(beginning.len(), 1024);
        assert!(beginning.iter().all(|sample| sample.is_finite()));
        let repeated: Vec<_> = track.skip(samples - 1024).take(1024).collect();
        assert_eq!(repeated, beginning);
    }

    #[test]
    fn missing_track_returns_an_error_without_opening_an_audio_device() {
        assert!(load_track(Path::new("assets/audio/archivo-inexistente.mp3")).is_err());
    }
}
