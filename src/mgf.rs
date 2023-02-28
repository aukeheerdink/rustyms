use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

use uom::num_traits::Zero;

use crate::{
    spectrum::{RawPeak, RawSpectrum},
    system::{charge::e, f64::*, mass::dalton, mass_over_charge::mz, time::s},
};

pub fn open(path: impl AsRef<Path>) -> Result<Vec<RawSpectrum>, ()> {
    let file = BufReader::new(File::open(path).map_err(|_| ())?);
    let mut current = RawSpectrum {
        title: String::new(),
        num_scans: 0,
        rt: Time::zero(),
        charge: Charge::zero(),
        mass: Mass::zero(),
        spectrum: Vec::new(),
    };
    let mut output = Vec::new();
    for (linenumber, line) in file.lines().enumerate() {
        let linenumber = linenumber + 1;
        let line = line.map_err(|_| ())?;
        match line.as_str() {
            "BEGIN IONS" | "" => (),
            "END IONS" => {
                output.push(current);
                current = RawSpectrum {
                    title: String::new(),
                    num_scans: 0,
                    rt: Time::zero(),
                    charge: Charge::zero(),
                    mass: Mass::zero(),
                    spectrum: Vec::new(),
                }
            }
            t if t.contains('=') => {
                let (key, value) = t.split_once('=').unwrap();
                match key {
                    "PEPMASS" => current.mass = Mass::new::<dalton>(value.parse().map_err(|_| ())?),
                    "CHARGE" => current.charge = parse_charge(value)?,
                    "RT" => current.rt = Time::new::<s>(value.parse().map_err(|_| ())?),
                    "TITLE" => current.title = value.to_owned(),
                    "NUM_SCANS" => current.num_scans = value.parse().map_err(|_| ())?,
                    _ => (),
                }
            }
            t if t.contains(' ') => {
                let split = t.split(' ').collect::<Vec<_>>();
                let mut peak = RawPeak {
                    mz: MassOverCharge::zero(),
                    intensity: 0.0,
                    charge: Charge::new::<e>(1.0),
                };
                if split.len() < 2 {
                    return Err(());
                }
                peak.mz = MassOverCharge::new::<mz>(split[0].parse().map_err(|_| ())?);
                peak.intensity = split[1].parse().map_err(|_| ())?;
                if split.len() >= 3 {
                    peak.charge = parse_charge(split[2])?;
                }
                current.spectrum.push(peak);
            }
            _ => {}
        }
    }
    Ok(output)
}

fn parse_charge(input: &str) -> Result<Charge, ()> {
    if input.ends_with('+') {
        Ok(Charge::new::<e>(
            input.trim_end_matches('+').parse().map_err(|_| ())?,
        ))
    } else if input.ends_with('-') {
        Ok(Charge::new::<e>(
            -input.trim_end_matches('-').parse().map_err(|_| ())?,
        ))
    } else {
        Ok(Charge::new::<e>(input.parse().map_err(|_| ())?))
    }
}

#[test]
fn test_open() {
    let spectra = open("data/example.mgf").unwrap();
    assert_eq!(spectra.len(), 1);
    assert_eq!(spectra[0].spectrum.len(), 5);
}