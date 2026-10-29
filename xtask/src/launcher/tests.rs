use super::*;

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| word.to_string()).collect()
}

fn request(words: &[&str]) -> Request {
    Request::parse(&args(words)).expect("a request the launcher takes")
}

fn launch(choice: Choice, build: Build, quality: Option<&'static str>) -> Launch {
    Launch {
        choice,
        build,
        quality,
        profiling: Profiling::Off,
    }
}

/// The menu driven by these lines, from `preselected` as the last launch; what it plays, and
/// what it showed.
fn drive(preselected: Launch, lines: &str) -> (Option<Launch>, String) {
    let last = Some(preselected.choice);
    let mut out = Vec::new();
    let chosen = menu(
        preselected,
        last,
        "Terra   main abc1234",
        &mut lines.as_bytes(),
        &mut out,
    );
    (chosen, String::from_utf8(out).unwrap())
}

#[test]
fn each_choice_is_the_play_experiment_with_the_game_flag_it_stands_for() {
    let game = |choice| launch(choice, Build::Optimised, None).game_args(&[]);
    assert_eq!(game(Choice::Continue), args(&["--experiment", "play"]));
    assert_eq!(
        game(Choice::New),
        args(&["--experiment", "play", "--fresh"])
    );
    assert_eq!(
        game(Choice::Creative),
        args(&["--experiment", "play", "--free-build"])
    );
    assert_eq!(
        game(Choice::Yard),
        args(&["--experiment", "play", "--level", "yard"])
    );
}

#[test]
fn a_quality_and_the_game_s_own_arguments_follow_the_choice() {
    let launch = launch(Choice::Yard, Build::Debug, Some("low"));
    assert_eq!(
        launch.game_args(&args(&["--time", "18:00"])),
        args(&[
            "--experiment",
            "play",
            "--level",
            "yard",
            "--quality",
            "low",
            "--time",
            "18:00"
        ])
    );
}

#[test]
fn flags_choose_what_the_menu_would() {
    assert_eq!(
        plan(
            &request(&["creative", "--build", "debug", "--quality", "high"]),
            true,
            None
        ),
        Plan::Play(launch(Choice::Creative, Build::Debug, Some("high")))
    );
    assert_eq!(
        plan(&request(&["new"]), true, None),
        Plan::Play(launch(Choice::New, Build::Optimised, None))
    );
    assert_eq!(
        plan(&request(&["--profiling", "report"]), false, None),
        Plan::Play(Launch {
            profiling: Profiling::Report,
            ..Launch::default()
        })
    );
}

#[test]
fn what_is_not_the_launcher_s_goes_to_the_game_as_make_play_args_passes_it() {
    let given = request(&["--time", "18:00", "--", "--quality", "low"]);
    assert_eq!(given.game, args(&["--time", "18:00", "--quality", "low"]));
    assert_eq!(given.quality, None);
    // A choice's word counts only first, so a game argument's value is never taken for one.
    assert_eq!(request(&["--level", "yard"]).choice, None);
}

#[test]
fn a_flag_the_launcher_cannot_read_is_refused_with_what_it_takes() {
    let why = Request::parse(&args(&["--build", "fast"])).unwrap_err();
    assert!(why.contains("optimised or debug"), "{why}");
    let why = Request::parse(&args(&["--quality", "epic"])).unwrap_err();
    assert!(why.contains("low, medium, high or ultra"), "{why}");
    let why = Request::parse(&args(&["--profiling", "loud"])).unwrap_err();
    assert!(why.contains("off, report or live"), "{why}");
}

#[test]
fn with_no_terminal_and_nothing_given_the_defaults_play_without_a_menu() {
    // An agent's or script's run: never the menu, and never what someone played last.
    let remembered = Some(launch(Choice::Yard, Build::Debug, Some("low")));
    assert_eq!(
        plan(&request(&[]), false, remembered),
        Plan::Play(Launch::default())
    );
    assert_eq!(
        Launch::default(),
        launch(Choice::Continue, Build::Optimised, None)
    );
}

#[test]
fn in_a_terminal_nothing_given_opens_the_menu_on_the_last_launch() {
    let remembered = launch(Choice::Creative, Build::Debug, None);
    assert_eq!(
        plan(&request(&[]), true, Some(remembered.clone())),
        Plan::Menu(remembered)
    );
    assert_eq!(
        plan(&request(&[]), true, None),
        Plan::Menu(Launch::default())
    );
}

#[test]
fn any_argument_skips_the_menu_and_menu_shows_it_anyway() {
    let remembered = Some(launch(Choice::Yard, Build::Optimised, None));
    assert_eq!(
        plan(&request(&["--time", "18:00"]), true, remembered.clone()),
        Plan::Play(Launch::default())
    );
    assert_eq!(
        plan(&request(&["--menu"]), false, remembered.clone()),
        Plan::Menu(launch(Choice::Yard, Build::Optimised, None))
    );
}

#[test]
fn last_plays_the_remembered_launch_with_any_flag_over_it() {
    let remembered = Some(launch(Choice::Creative, Build::Debug, Some("ultra")));
    assert_eq!(
        plan(&request(&["--last"]), false, remembered.clone()),
        Plan::Play(launch(Choice::Creative, Build::Debug, Some("ultra")))
    );
    assert_eq!(
        plan(
            &request(&["--last", "--quality", "auto"]),
            false,
            remembered
        ),
        Plan::Play(launch(Choice::Creative, Build::Debug, None))
    );
}

#[test]
fn the_launch_is_kept_and_read_back_and_a_new_game_is_kept_as_continue() {
    let played = Launch {
        profiling: Profiling::Report,
        ..launch(Choice::Yard, Build::Debug, Some("medium"))
    };
    assert_eq!(Launch::from_json(&played.remembered().to_json()), played);
    let new = launch(Choice::New, Build::Optimised, None);
    assert_eq!(
        Launch::from_json(&new.remembered().to_json()).choice,
        Choice::Continue
    );
    // A file damaged or from elsewhere opens on the defaults rather than failing the launch.
    assert_eq!(Launch::from_json("not json"), Launch::default());
    assert_eq!(
        Launch::from_json(r#"{"choice": "flying", "quality": "low"}"#),
        launch(Choice::Continue, Build::Optimised, Some("low"))
    );
}

#[test]
fn enter_plays_the_preselected_launch_and_the_screen_marks_it() {
    let last = launch(Choice::Creative, Build::Optimised, None);
    let (chosen, shown) = drive(last.clone(), "\n");
    assert_eq!(chosen, Some(last));
    assert!(shown.contains("Enter plays Creative."), "{shown}");
    let marked = shown.lines().find(|line| line.contains("<- last time"));
    assert!(
        marked.is_some_and(|line| line.contains("Creative")),
        "{shown}"
    );
}

#[test]
fn a_number_plays_its_choice() {
    let (chosen, _) = drive(Launch::default(), "4\n");
    assert_eq!(chosen.map(|launch| launch.choice), Some(Choice::Yard));
}

#[test]
fn a_new_game_asks_before_it_deletes_the_save() {
    let (chosen, shown) = drive(Launch::default(), "2\nn\nq\n");
    assert_eq!(chosen, None);
    assert!(shown.contains("deletes your saved world"), "{shown}");
    assert!(shown.contains("Your world is kept."), "{shown}");
    let (chosen, _) = drive(Launch::default(), "2\ny\n");
    assert_eq!(chosen.map(|launch| launch.choice), Some(Choice::New));
}

#[test]
fn options_change_the_build_quality_and_profiling_by_word_or_its_first_letters() {
    let (chosen, shown) = drive(Launch::default(), "o\nd\nlow\nr\n\n");
    assert_eq!(
        chosen,
        Some(Launch {
            profiling: Profiling::Report,
            ..launch(Choice::Continue, Build::Debug, Some("low"))
        })
    );
    assert!(
        shown.contains("Build: debug   Quality: low   Profiling: report"),
        "{shown}"
    );
    // Enter keeps each; a word it does not know says so and keeps it too.
    let (chosen, shown) = drive(Launch::default(), "o\n\nepic\n\n3\n");
    assert_eq!(
        chosen,
        Some(launch(Choice::Creative, Build::Optimised, None))
    );
    assert!(shown.contains("`epic` is not a preset"), "{shown}");
    // Live is Tracy's, and says so on the screen.
    let (chosen, shown) = drive(Launch::default(), "o\n\n\nlive\n\n");
    assert_eq!(chosen.map(|launch| launch.profiling), Some(Profiling::Live));
    assert!(shown.contains("Profiling: live (Tracy)"), "{shown}");
}

#[test]
fn q_or_the_end_of_input_quits_without_playing() {
    assert_eq!(drive(Launch::default(), "q\n").0, None);
    assert_eq!(drive(Launch::default(), "").0, None);
    let (chosen, shown) = drive(Launch::default(), "9\nq\n");
    assert_eq!(chosen, None);
    assert!(shown.contains("`9` is not one of these"), "{shown}");
}

#[test]
fn a_first_launch_marks_no_last_time() {
    let mut out = Vec::new();
    menu(
        Launch::default(),
        None,
        "Terra",
        &mut "q\n".as_bytes(),
        &mut out,
    );
    let shown = String::from_utf8(out).unwrap();
    assert!(!shown.contains("last time"), "{shown}");
    assert!(shown.contains("Enter plays Continue."), "{shown}");
}
