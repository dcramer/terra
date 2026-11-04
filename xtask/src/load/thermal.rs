//! The CPU package's temperature, read beside its clock (D185), so a collapse of the clock
//! says whether the package was at its TjMax just before: the render worker's i9-14900K
//! reached 96-100 °C (TjMax 100) in the 2 s before every one of 20 collapses, then clamped
//! (forest-hitch j1). That is the box's cooling to check, which no change to the game fixes.
//! Linux only, from files any user may read: coretemp's (or k10temp's) hwmon sensor, whose
//! `_crit` is TjMax, else the `x86_pkg_temp` thermal zone.

use std::path::{Path, PathBuf};

/// The package counts as at its TjMax within this many degrees of it: the chip and thermald
/// clamp as it nears its trip, not only on it.
pub(crate) const NEAR_C: f32 = 5.0;
/// How long before a collapse its package's hottest is looked for: the worker's fell from
/// 96-100 °C to 57-69 °C as the clamp took hold, before its clock read the fall.
pub(crate) const BEFORE_S: f32 = 2.0;

/// The package's sensor: the file its temperature is read from, in millidegrees, and its
/// TjMax in °C where the chip says one.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Sensor {
    pub file: PathBuf,
    pub tjmax_c: Option<f32>,
}

/// A sysfs temperature, millidegrees Celsius, in °C.
pub(crate) fn parse_c(text: &str) -> Option<f32> {
    text.trim().parse::<i64>().ok().map(|m| m as f32 / 1000.0)
}

fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|text| text.trim().to_string())
}

/// The package's sensor under `class` (`/sys/class`): an hwmon labelled `Package id 0`
/// (coretemp) or `Tctl` (k10temp), with its `_crit`; else the `x86_pkg_temp` thermal zone,
/// which says no TjMax. Directories are taken in name order, so the same box finds the same.
pub(crate) fn sensor(class: &Path) -> Option<Sensor> {
    let sorted = |dir: PathBuf| -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
            .map(|entries| entries.filter_map(|e| e.ok().map(|e| e.path())).collect())
            .unwrap_or_default();
        paths.sort();
        paths
    };
    for label in ["Package id 0", "Tctl"] {
        for hwmon in sorted(class.join("hwmon")) {
            for file in sorted(hwmon.clone()) {
                let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let Some(stem) = name.strip_suffix("_label") else {
                    continue;
                };
                if read(&file).as_deref() != Some(label) {
                    continue;
                }
                let input = hwmon.join(format!("{stem}_input"));
                if input.exists() {
                    let tjmax_c =
                        read(&hwmon.join(format!("{stem}_crit"))).and_then(|text| parse_c(&text));
                    return Some(Sensor {
                        file: input,
                        tjmax_c,
                    });
                }
            }
        }
    }
    sorted(class.join("thermal"))
        .into_iter()
        .find(|zone| read(&zone.join("type")).as_deref() == Some("x86_pkg_temp"))
        .map(|zone| Sensor {
            file: zone.join("temp"),
            tjmax_c: None,
        })
}

impl Sensor {
    /// The package now, °C.
    pub(crate) fn now_c(&self) -> Option<f32> {
        parse_c(&read(&self.file)?)
    }
}

/// The package's hottest in the `BEFORE_S` up to each fall, from its readings (each its
/// second from the run's start and its °C); none for a fall with no reading then.
pub(crate) fn hottest_before(falls_s: &[f32], readings: &[(f32, f32)]) -> Vec<Option<f32>> {
    falls_s
        .iter()
        .map(|&fall| {
            readings
                .iter()
                .filter(|&&(at, _)| (fall - BEFORE_S..=fall).contains(&at))
                .map(|&(_, c)| c)
                .reduce(f32::max)
        })
        .collect()
}

/// What a run's clock collapses were, by the package's hottest before each and its TjMax
/// (D556: whose the slow frames are, and what to check).
pub(crate) fn cause(hottest: &[Option<f32>], tjmax_c: Option<f32>) -> String {
    let read: Vec<f32> = hottest.iter().flatten().copied().collect();
    let Some(most) = read.iter().copied().reduce(f32::max) else {
        return "its power management, not the game".to_string();
    };
    let Some(tjmax) = tjmax_c else {
        return format!(
            "its power management, not the game: package at most {most:.0} °C before them, its TjMax unread"
        );
    };
    let hot: Vec<f32> = read
        .iter()
        .copied()
        .filter(|&c| c >= tjmax - NEAR_C)
        .collect();
    if hot.is_empty() {
        return format!(
            "its power management, not the game: package at most {most:.0} °C before them, under its TjMax {tjmax:.0} °C"
        );
    }
    let least = hot.iter().copied().fold(f32::MAX, f32::min);
    let at = if least.round() == most.round() {
        format!("{most:.0} °C")
    } else {
        format!("{least:.0}-{most:.0} °C")
    };
    let which = if hot.len() == hottest.len() {
        "before each".to_string()
    } else {
        format!("before {} of them", hot.len())
    };
    format!(
        "thermal: package at {at} {which}, its TjMax {tjmax:.0} °C; check the box's cooling, not the game"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fake `/sys/class` in the temporary folder, its files as given.
    fn class(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("xtask-thermal-{}-{name}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        for (path, text) in files {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, format!("{text}\n")).unwrap();
        }
        root
    }

    /// coretemp's package sensor and its TjMax, past a core's and another hwmon; the
    /// thermal zone where no hwmon says; none where neither is.
    #[test]
    fn finds_the_package_sensor_and_its_tjmax() {
        let both = class(
            "both",
            &[
                ("hwmon/hwmon0/name", "acpitz"),
                ("hwmon/hwmon0/temp1_input", "27800"),
                ("hwmon/hwmon4/name", "coretemp"),
                ("hwmon/hwmon4/temp2_label", "Core 0"),
                ("hwmon/hwmon4/temp2_input", "61000"),
                ("hwmon/hwmon4/temp1_label", "Package id 0"),
                ("hwmon/hwmon4/temp1_input", "77000"),
                ("hwmon/hwmon4/temp1_crit", "100000"),
                ("thermal/thermal_zone3/type", "x86_pkg_temp"),
                ("thermal/thermal_zone3/temp", "77000"),
            ],
        );
        let found = sensor(&both).unwrap();
        assert_eq!(found.file, both.join("hwmon/hwmon4/temp1_input"));
        assert_eq!((found.tjmax_c, found.now_c()), (Some(100.0), Some(77.0)));
        let zone = class(
            "zone",
            &[
                ("thermal/thermal_zone0/type", "acpitz"),
                ("thermal/thermal_zone0/temp", "27800"),
                ("thermal/thermal_zone3/type", "x86_pkg_temp"),
                ("thermal/thermal_zone3/temp", "96500"),
            ],
        );
        let found = sensor(&zone).unwrap();
        assert_eq!((found.tjmax_c, found.now_c()), (None, Some(96.5)));
        let none = class("none", &[("thermal/x", "")]);
        assert_eq!(sensor(&none), None);
        for root in [both, zone, none] {
            std::fs::remove_dir_all(root).ok();
        }
    }

    /// The hottest reading in the 2 s up to each fall, not after it, nor long before.
    #[test]
    fn takes_the_package_hottest_just_before_each_fall() {
        let readings: Vec<(f32, f32)> = vec![
            (1.0, 99.0),
            (8.5, 97.0),
            (9.5, 100.0),
            (10.0, 62.0),
            (10.5, 101.0),
            (20.0, 70.0),
        ];
        assert_eq!(
            hottest_before(&[10.0, 21.0, 40.0], &readings),
            vec![Some(100.0), Some(70.0), None]
        );
    }

    /// Thermal when the package was at or within 5 °C of TjMax before the falls; the power
    /// management's otherwise, with the temperature said where it was read.
    #[test]
    fn a_collapse_at_tjmax_is_thermal() {
        let tj = Some(100.0);
        assert_eq!(
            cause(&[Some(100.0), Some(100.2)], tj),
            "thermal: package at 100 °C before each, its TjMax 100 °C; check the box's cooling, not the game"
        );
        assert!(
            cause(&[Some(96.0), Some(100.0)], tj)
                .starts_with("thermal: package at 96-100 °C before each")
        );
        assert!(
            cause(&[Some(97.0), Some(70.0), None], tj)
                .starts_with("thermal: package at 97 °C before 1 of them")
        );
        assert_eq!(
            cause(&[Some(72.0)], tj),
            "its power management, not the game: package at most 72 °C before them, under its TjMax 100 °C"
        );
        assert!(cause(&[Some(99.0)], None).ends_with("its TjMax unread"));
        assert_eq!(cause(&[None], tj), "its power management, not the game");
        assert_eq!(cause(&[], None), "its power management, not the game");
    }
}
