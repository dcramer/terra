use super::*;

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| word.to_string()).collect()
}

fn request(words: &[&str]) -> Request {
    Request::parse(&args(words)).expect("a request the launcher takes")
}

fn launch(choice: Option<Choice>, build: Build) -> Launch {
    Launch {
        choice,
        build,
        profiling: Profiling::Off,
    }
}

/// The menu driven by these lines, from `preselected`; what it plays, and what it showed.
fn drive(preselected: Launch, lines: &str) -> (Option<Launch>, String) {
    let mut out = Vec::new();
    let chosen = menu(
        preselected,
        "Terra   main abc1234",
        &mut lines.as_bytes(),
        &mut out,
    );
    (chosen, String::from_utf8(out).unwrap())
}

#[test]
fn each_word_goes_straight_in_with_the_game_flag_it_stands_for_and_none_opens_the_title() {
    let game = |choice| launch(choice, Build::Optimised).game_args(&[]);
    assert_eq!(
        game(Some(Choice::Continue)),
        args(&["--experiment", "play"])
    );
    assert_eq!(
        game(Some(Choice::New)),
        args(&["--experiment", "play", "--fresh"])
    );
    assert_eq!(
        game(Some(Choice::Creative)),
        args(&["--experiment", "play", "--creative"])
    );
    assert_eq!(
        game(Some(Choice::Yard)),
        args(&["--experiment", "play", "--level", "yard"])
    );
    assert_eq!(game(None), args(&["--experiment", "play", "--title"]));
}

#[test]
fn the_game_s_own_arguments_follow_the_choice() {
    let launch = launch(Some(Choice::Yard), Build::Debug);
    assert_eq!(
        launch.game_args(&args(&["--time", "18:00", "--quality", "low"])),
        args(&[
            "--experiment",
            "play",
            "--level",
            "yard",
            "--time",
            "18:00",
            "--quality",
            "low"
        ])
    );
}

#[test]
fn flags_choose_what_to_play_and_how() {
    assert_eq!(
        plan(&request(&["creative", "--build", "debug"]), true, None),
        Plan::Play(launch(Some(Choice::Creative), Build::Debug))
    );
    assert_eq!(
        plan(&request(&["new"]), true, None),
        Plan::Play(launch(Some(Choice::New), Build::Optimised))
    );
    assert_eq!(
        plan(&request(&["--profiling", "report"]), false, None),
        Plan::Play(Launch {
            choice: Some(Choice::Continue),
            profiling: Profiling::Report,
            ..Launch::default()
        })
    );
}

#[test]
fn what_is_not_the_launcher_s_goes_to_the_game_as_make_play_args_passes_it() {
    let given = request(&["--time", "18:00", "--quality", "low", "--", "--mute"]);
    assert_eq!(
        given.game,
        args(&["--time", "18:00", "--quality", "low", "--mute"])
    );
    // A choice's word counts only first, so a game argument's value is never taken for one.
    assert_eq!(request(&["--level", "yard"]).choice, None);
}

#[test]
fn a_flag_the_launcher_cannot_read_is_refused_with_what_it_takes() {
    let why = Request::parse(&args(&["--build", "fast"])).unwrap_err();
    assert!(why.contains("optimised or debug"), "{why}");
    let why = Request::parse(&args(&["--profiling", "loud"])).unwrap_err();
    assert!(why.contains("off, report or live"), "{why}");
}

#[test]
fn a_scripted_run_never_meets_the_menu_or_the_title_screen() {
    // An agent's or script's run (no terminal): Continue, straight in, never what someone
    // played last, whatever it was given.
    let remembered = Some(launch(None, Build::Debug));
    for given in [&[][..], &["--time", "18:00"][..], &["--last"][..]] {
        let Plan::Play(launch) = plan(&request(given), false, remembered.clone()) else {
            panic!("{given:?} opened the menu");
        };
        assert_eq!(launch.choice, Some(Choice::Continue), "{given:?}");
        assert!(!launch.game_args(&[]).contains(&TITLE.to_string()));
    }
}

#[test]
fn in_a_terminal_nothing_given_opens_the_menu_then_the_title_screen() {
    let remembered = launch(None, Build::Debug);
    assert_eq!(
        plan(&request(&[]), true, Some(remembered.clone())),
        Plan::Menu(remembered)
    );
    assert_eq!(
        plan(&request(&[]), true, None),
        Plan::Menu(Launch::default())
    );
    // Game arguments alone skip the menu, but a person still chooses on the title screen.
    assert_eq!(
        plan(&request(&["--time", "18:00"]), true, None),
        Plan::Play(launch(None, Build::Optimised))
    );
}

#[test]
fn menu_shows_it_anyway_and_last_plays_the_remembered_options() {
    let remembered = Some(launch(None, Build::Debug));
    assert_eq!(
        plan(&request(&["--menu"]), false, remembered.clone()),
        Plan::Menu(launch(None, Build::Debug))
    );
    assert_eq!(
        plan(&request(&["yard", "--last"]), true, remembered),
        Plan::Play(launch(Some(Choice::Yard), Build::Debug))
    );
}

#[test]
fn the_options_are_kept_and_read_back_and_an_older_file_s_choices_passed_over() {
    let played = Launch {
        profiling: Profiling::Report,
        ..launch(None, Build::Debug)
    };
    assert_eq!(Launch::from_json(&played.to_json()), played);
    let kept = Launch {
        choice: Some(Choice::New),
        ..Launch::default()
    }
    .to_json();
    assert!(!kept.contains("new") && !kept.contains("choice"), "{kept}");
    // A file damaged or from elsewhere opens on the defaults rather than failing the launch.
    assert_eq!(Launch::from_json("not json"), Launch::default());
    assert_eq!(
        Launch::from_json(r#"{"choice": "yard", "quality": "low", "build": "debug"}"#),
        launch(None, Build::Debug)
    );
}

#[test]
fn enter_plays_the_options_on_the_title_screen() {
    let (chosen, shown) = drive(launch(None, Build::Debug), "\n");
    assert_eq!(chosen, Some(launch(None, Build::Debug)));
    assert!(shown.contains("opens on its title screen"), "{shown}");
    assert!(
        !shown.contains("Continue"),
        "the menu no longer chooses: {shown}"
    );
}

#[test]
fn options_change_the_build_and_profiling_by_word_or_its_first_letters() {
    let (chosen, shown) = drive(Launch::default(), "o\nd\nr\n\n");
    assert_eq!(
        chosen,
        Some(Launch {
            profiling: Profiling::Report,
            ..launch(None, Build::Debug)
        })
    );
    assert!(
        shown.contains("Build: debug   Profiling: report"),
        "{shown}"
    );
    // Enter keeps each; a word it does not know says so and keeps it too.
    let (chosen, shown) = drive(Launch::default(), "o\nfast\n\n\n");
    assert_eq!(chosen, Some(Launch::default()));
    assert!(
        shown.contains("`fast` is not optimised or debug"),
        "{shown}"
    );
    // Live is Tracy's, and says so on the screen.
    let (chosen, shown) = drive(Launch::default(), "o\n\nlive\n\n");
    assert_eq!(chosen.map(|launch| launch.profiling), Some(Profiling::Live));
    assert!(shown.contains("Profiling: live (Tracy)"), "{shown}");
}

#[test]
fn q_or_the_end_of_input_quits_without_playing() {
    assert_eq!(drive(Launch::default(), "q\n").0, None);
    assert_eq!(drive(Launch::default(), "").0, None);
    let (chosen, shown) = drive(Launch::default(), "2\nq\n");
    assert_eq!(chosen, None);
    assert!(shown.contains("`2` is not one of these"), "{shown}");
}

/// The game's own number for a restart, read from where it is defined
/// (`terra_game::play::title`), so the launcher and the game cannot drift apart.
fn games_code(name: &str) -> i32 {
    let source = include_str!("../../../crates/terra_game/src/play/title.rs");
    let line = source
        .lines()
        .find(|line| line.starts_with(&format!("pub const {name}: u8 = ")))
        .unwrap_or_else(|| panic!("terra_game's title.rs defines {name}"));
    line.trim_end_matches(';')
        .rsplit(' ')
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

/// What the launcher plays after a game that ended with `code`, given as a script gives it:
/// no choice word, the game's title screen and a few presses.
fn after(code: Option<i32>) -> Option<Vec<String>> {
    let mut launch = launch(Some(Choice::Continue), Build::Optimised);
    let mut extra = args(&["--title", "--press", "enter@140", "--shots", "a@120,400"]);
    again(code, &mut launch, &mut extra).then(|| launch.game_args(&extra))
}

#[test]
fn the_game_asking_to_start_again_plays_the_title_screen_or_the_yard() {
    assert_eq!(games_code("AGAIN_ON_TITLE"), AGAIN_ON_TITLE);
    assert_eq!(games_code("AGAIN_ON_YARD"), AGAIN_ON_YARD);
    let rest = ["--press", "enter@140", "--shots", "a@120,400"];
    // The yard, and only the yard: the first game's `--title` is not given again.
    let yard = [&["--experiment", "play", "--level", "yard"][..], &rest].concat();
    assert_eq!(after(Some(games_code("AGAIN_ON_YARD"))), Some(args(&yard)));
    // The title screen, asked for once.
    let title = [&["--experiment", "play", "--title"][..], &rest].concat();
    assert_eq!(
        after(Some(games_code("AGAIN_ON_TITLE"))),
        Some(args(&title))
    );
    // Ending well, failing, panicking, or killed: the session is over.
    for code in [None, Some(0), Some(1), Some(101)] {
        assert_eq!(after(code), None, "{code:?}");
    }
}
