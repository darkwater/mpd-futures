use std::str::FromStr;

#[derive(Clone, Debug, Default)]
pub struct Status {
    pub volume:         Option<u64>,
    pub repeat:         Option<bool>,
    pub random:         Option<bool>,
    pub single:         Option<bool>,
    pub consume:        Option<bool>,
    pub playlist:       Option<usize>,
    pub playlistlength: Option<usize>,
    pub state:          Option<State>,
    pub playing:        Option<bool>, // Convenience fields
    pub paused:         Option<bool>, //
    pub stopped:        Option<bool>, //
    pub song:           Option<usize>,
    pub songid:         Option<usize>,
    pub nextsong:       Option<usize>,
    pub nextsongid:     Option<usize>,
    // time
    pub elapsed:        Option<f64>,
    pub duration:       Option<f64>,
    pub bitrate:        Option<u64>,
    pub xfade:          Option<u64>,
    // mixrampdb
    // mixrampdelay
    // audio
    // updating_db
    // error
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Pause,
    Play,
    Stop,
}

impl FromStr for State {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use State::*;
        match s {
            "pause" => Ok(Pause),
            "play"  => Ok(Play),
            "stop"  => Ok(Stop),
            _       => Err(()),
        }
    }
}
