use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Quick enough for a test: a beat every 20 ms, watched for 150.
const QUICK: Timing = Timing {
    beat: Duration::from_millis(20),
    settle: Duration::from_millis(150),
};

/// A folder of its own for a test, empty.
fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "terra-single-instance-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn copy(pid: u32, os: &str, command: &str) -> Record {
    Record {
        pid,
        os: os.to_string(),
        worktree: format!("/work/{pid}"),
        command: command.to_string(),
        since: 1_793_600_709,
        beat: 0,
    }
}

fn take(dir: &Path, multirun: bool, me: &Record) -> Result<Held, Refusal> {
    take_in(dir, multirun, me, QUICK).expect("the folder is usable")
}

#[test]
fn a_second_copy_is_refused_saying_who_holds_it_and_what_to_do() {
    let dir = folder("second");
    let first = take(
        &dir,
        false,
        &copy(11, "linux", "terra_sandbox --shot a.png"),
    )
    .unwrap();
    let refusal = take(&dir, false, &copy(12, "linux", "terra_sandbox"))
        .err()
        .unwrap();
    assert_eq!(
        refusal.holders,
        [Some(copy(11, "linux", "terra_sandbox --shot a.png"))]
    );
    let said = refusal.to_string();
    assert!(said.starts_with(REFUSAL), "{said}");
    for words in [
        "pid 11 on linux, in /work/11, since 06:25:09 UTC: terra_sandbox --shot a.png",
        "wait for it to end",
        SWITCH,
        "D319",
    ] {
        assert!(said.contains(words), "{words:?} missing from: {said}");
    }
    drop(first);
}

#[test]
fn the_switch_allows_exactly_one_more_copy() {
    let dir = folder("switch");
    let first = take(&dir, false, &copy(21, "linux", "host")).unwrap();
    let second = take(&dir, true, &copy(22, "linux", "join")).unwrap();
    assert_eq!((first.slot, second.slot), (0, 1));
    let third = take(&dir, true, &copy(23, "linux", "third")).err().unwrap();
    assert_eq!(third.holders.len(), 2);
    assert!(third.to_string().contains("nothing allows a third"));
    assert!(take(&dir, false, &copy(24, "linux", "plain")).is_err());
}

#[test]
fn a_copy_without_the_switch_is_refused_while_a_second_copy_runs() {
    let dir = folder("second-left");
    let first = take(&dir, false, &copy(31, "linux", "host")).unwrap();
    let second = take(&dir, true, &copy(32, "linux", "join")).unwrap();
    // The first ends; the one that passed the switch still runs.
    drop(first);
    let refusal = take(&dir, false, &copy(33, "linux", "plain"))
        .err()
        .unwrap();
    assert_eq!(refusal.holders, [Some(copy(32, "linux", "join"))]);
    drop(second);
    assert!(take(&dir, false, &copy(34, "linux", "plain")).is_ok());
}

#[test]
fn a_dead_copys_slot_is_taken_over() {
    let dir = folder("dead");
    // Dropping a held slot frees its lock as a crash would, and leaves its record behind.
    drop(take(&dir, false, &copy(41, "linux", "crashed")).unwrap());
    assert!(record_path(&dir, 0).exists());
    let next = take(&dir, false, &copy(42, "linux", "next")).unwrap();
    assert_eq!(next.slot, 0);
    let record = std::fs::read_to_string(record_path(&dir, 0)).unwrap();
    assert_eq!(Record::parse(&record).map(|r| r.pid), Some(42));
}

/// A record written as a copy on the other side of the WSL line writes it, its lock unseen.
fn other_side(dir: &Path, record: &Record) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(record_path(dir, 0), record.text()).unwrap();
}

#[test]
fn a_copy_across_the_wsl_line_holds_its_slot_while_it_beats() {
    let dir = folder("across-live");
    let windows = copy(51, "windows", "terra_sandbox.exe --experiment play");
    other_side(&dir, &windows);
    let beating = Arc::new(AtomicBool::new(true));
    let (still, path, mut record) = (beating.clone(), record_path(&dir, 0), windows.clone());
    let beat = std::thread::spawn(move || {
        while still.load(Ordering::Relaxed) {
            std::thread::sleep(QUICK.beat);
            record.beat += 1;
            std::fs::write(&path, record.text()).unwrap();
        }
    });
    let refusal = take(&dir, false, &copy(52, "linux", "terra_sandbox")).err();
    beating.store(false, Ordering::Relaxed);
    beat.join().unwrap();
    let holder = refusal.expect("refused").holders[0]
        .clone()
        .expect("its record");
    assert_eq!((holder.pid, holder.os.as_str()), (51, "windows"));
}

#[test]
fn a_copy_across_the_wsl_line_that_stopped_beating_is_taken_over() {
    let dir = folder("across-dead");
    other_side(&dir, &copy(61, "windows", "terra_sandbox.exe"));
    let next = take(&dir, false, &copy(62, "linux", "terra_sandbox")).unwrap();
    assert_eq!(next.slot, 0);
}

#[test]
fn a_copy_that_closed_cleanly_keeps_the_other_side_of_the_wsl_line_from_waiting() {
    // A watch long enough to tell apart from a start that skipped it.
    let slow = Timing {
        beat: QUICK.beat,
        settle: Duration::from_secs(2),
    };
    let dir = folder("released");
    let linux = take_in(&dir, false, &copy(91, "linux", "terra_sandbox"), slow)
        .unwrap()
        .unwrap();
    linux.release();
    // No beat left running brings the record back.
    std::thread::sleep(QUICK.beat * 4);
    assert!(
        !record_path(&dir, 0).exists(),
        "the record outlived its copy"
    );
    let started = Instant::now();
    let windows = take_in(&dir, false, &copy(92, "windows", "terra_sandbox.exe"), slow)
        .unwrap()
        .unwrap();
    assert_eq!(windows.slot, 0);
    assert!(
        started.elapsed() < slow.settle / 2,
        "the start waited {:?} on a record from a copy that had closed",
        started.elapsed()
    );
}

#[test]
fn a_holder_keeps_its_record_beating() {
    let dir = folder("beat");
    let held = take(&dir, false, &copy(71, "linux", "beating")).unwrap();
    std::thread::sleep(QUICK.beat * 6);
    let record = Record::parse(&std::fs::read_to_string(record_path(&dir, 0)).unwrap()).unwrap();
    assert!(record.beat >= 2, "beat {}", record.beat);
    drop(held);
}

#[test]
fn a_record_reads_back_as_written() {
    let mut record = copy(81, "linux", "terra_sandbox --tune a=1 'two words'");
    record.beat = 9;
    assert_eq!(Record::parse(&record.text()), Some(record));
    assert_eq!(Record::parse("pid=1\n"), None);
}
