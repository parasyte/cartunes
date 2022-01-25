use super::*;
use winit::dpi::PhysicalSize;

fn create_ordered_multimap(list: &[(&str, &str)]) -> ListOrderedMultimap<String, String> {
    list.iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn test_load_dir() {
    let mut config = Config::new("/tmp/some/path.toml", PhysicalSize::new(0, 0));
    config.update_setups_path("./fixtures");
    let mut warnings = VecDeque::new();
    let setups = Setups::new(&mut warnings, &config);

    assert!(warnings.is_empty());

    let tracks = setups.tracks();
    let cars = &tracks["Centripetal Circuit"]["Skip Barber Formula 2000"];
    assert_eq!(cars.len(), 1);
    let SetupInfo {
        setup: skip_barber,
        name: file_name,
        ..
    } = &cars[0];
    assert_eq!(file_name, "skip_barber_centripetal");
    assert_eq!(skip_barber.keys().len(), 6);

    let cars = &tracks["Charlotte Motor Speedway"]["Global Mazda MX-5 Cup"];
    assert_eq!(cars.len(), 1);
    let SetupInfo {
        setup: mx5,
        name: file_name,
        ..
    } = &cars[0];
    assert_eq!(file_name, "mx5_charlotte_legends_oval");
    assert_eq!(mx5.keys().len(), 6);

    let cars = &tracks["Circuit des 24 Heures du Mans - 24 Heures du Mans"]["Dallara P217"];
    assert_eq!(cars.len(), 1);
    let SetupInfo {
        setup: dallara,
        name: file_name,
        ..
    } = &cars[0];
    assert_eq!(file_name, "iracing_lemans_default");
    assert_eq!(dallara.keys().len(), 18);

    let cars = &tracks["Nürburgring Combined"]["Porsche 911 GT3 R"];
    assert_eq!(cars.len(), 1);
    let SetupInfo {
        setup: porche911,
        name: file_name,
        ..
    } = &cars[0];
    assert_eq!(file_name, "baseline");
    assert_eq!(porche911.keys().len(), 12);

    let cars = &tracks["Watkins Glen International"]["Mercedes-AMG W12 E Performance"];
    assert_eq!(cars.len(), 1);
    let SetupInfo {
        setup: mercedes,
        name: file_name,
        ..
    } = &cars[0];
    assert_eq!(file_name, "iracing_w12_baseline_glenboot");
    assert_eq!(mercedes.keys().len(), 16);

    assert_eq!(setups.tracks().len(), 5);
}

#[test]
fn test_setup_skip_barber() {
    let config = Config::new("/tmp/some/path.toml", PhysicalSize::new(0, 0));
    let (track_name, car_name, setup) =
        setup_from_html("./fixtures/skip_barber_centripetal.htm", &config).unwrap();

    assert_eq!(track_name, "Centripetal Circuit".to_string());
    assert_eq!(car_name, "Skip Barber Formula 2000".to_string());
    assert_eq!(setup.keys().len(), 6);

    // Front
    let expected = create_ordered_multimap(&[("Brake bias", "54%")]);
    let front = setup.get("Front").unwrap();
    assert_eq!(front, &expected);

    // Left Front
    let expected = create_ordered_multimap(&[
        ("Cold pressure", "25.0 psi"),
        ("Last hot pressure", "25.0 psi"),
        ("Last temps O M I", "119F"),
        ("Last temps O M I", "119F"),
        ("Last temps O M I", "119F"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
        ("Corner weight", "301 lbs"),
        ("Ride height", "1.95 in"),
        ("Spring perch offset", "5 x 1/16 in."),
        ("Camber", "-1.6 deg"),
        ("Caster", "+12.2 deg"),
    ]);
    let left_front = setup.get("Left Front").unwrap();
    assert_eq!(left_front, &expected);

    // Left Rear
    let expected = create_ordered_multimap(&[
        ("Cold pressure", "25.0 psi"),
        ("Last hot pressure", "25.0 psi"),
        ("Last temps O M I", "119F"),
        ("Last temps O M I", "119F"),
        ("Last temps O M I", "119F"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
        ("Corner weight", "438 lbs"),
        ("Ride height", "3.20 in"),
        ("Camber", "-2.1 deg"),
    ]);
    let left_rear = setup.get("Left Rear").unwrap();
    assert_eq!(left_rear, &expected);

    // Right Front
    let expected = create_ordered_multimap(&[
        ("Cold pressure", "25.0 psi"),
        ("Last hot pressure", "25.0 psi"),
        ("Last temps I M O", "119F"),
        ("Last temps I M O", "119F"),
        ("Last temps I M O", "119F"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
        ("Corner weight", "301 lbs"),
        ("Ride height", "1.95 in"),
        ("Spring perch offset", "5 x 1/16 in."),
        ("Camber", "-1.6 deg"),
        ("Caster", "+12.2 deg"),
    ]);
    let right_front = setup.get("Right Front").unwrap();
    assert_eq!(right_front, &expected);

    // Right Rear
    let expected = create_ordered_multimap(&[
        ("Cold pressure", "25.0 psi"),
        ("Last hot pressure", "25.0 psi"),
        ("Last temps I M O", "119F"),
        ("Last temps I M O", "119F"),
        ("Last temps I M O", "119F"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
        ("Corner weight", "438 lbs"),
        ("Ride height", "3.20 in"),
        ("Camber", "-2.1 deg"),
    ]);
    let right_rear = setup.get("Right Rear").unwrap();
    assert_eq!(right_rear, &expected);

    // Rear
    let expected = create_ordered_multimap(&[("Fuel level", "4.2 gal"), ("Anti-roll bar", "6")]);
    let rear = setup.get("Rear").unwrap();
    assert_eq!(rear, &expected);
}

#[test]
fn test_setup_mx5() {
    let config = Config::new("/tmp/some/path.toml", PhysicalSize::new(0, 0));
    let (track_name, car_name, setup) =
        setup_from_html("./fixtures/mx5_charlotte_legends_oval.htm", &config).unwrap();

    assert_eq!(track_name, "Charlotte Motor Speedway".to_string());
    assert_eq!(car_name, "Global Mazda MX-5 Cup".to_string());
    assert_eq!(setup.keys().len(), 6);

    // Front
    let expected = create_ordered_multimap(&[
        ("Toe-in", r#"-0/16""#),
        ("Cross weight", "50.0%"),
        ("Anti-roll bar", "Firm"),
    ]);
    let front = setup.get("Front").unwrap();
    assert_eq!(front, &expected);

    // Left Front
    let expected = create_ordered_multimap(&[
        ("Cold pressure", "30.0 psi"),
        ("Last hot pressure", "30.0 psi"),
        ("Last temps O M I", "103F"),
        ("Last temps O M I", "103F"),
        ("Last temps O M I", "103F"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
        ("Corner weight", "605 lbs"),
        ("Ride height", "4.83 in"),
        ("Spring perch offset", r#"2.563""#),
        ("Bump stiffness", "+10 clicks"),
        ("Rebound stiffness", "+8 clicks"),
        ("Camber", "-2.7 deg"),
    ]);
    let left_front = setup.get("Left Front").unwrap();
    assert_eq!(left_front, &expected);

    // Left Rear
    let expected = create_ordered_multimap(&[
        ("Cold pressure", "30.0 psi"),
        ("Last hot pressure", "30.0 psi"),
        ("Last temps O M I", "103F"),
        ("Last temps O M I", "103F"),
        ("Last temps O M I", "103F"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
        ("Corner weight", "540 lbs"),
        ("Ride height", "4.86 in"),
        ("Spring perch offset", r#"1.625""#),
        ("Bump stiffness", "+8 clicks"),
        ("Rebound stiffness", "+10 clicks"),
        ("Camber", "-2.7 deg"),
    ]);
    let left_rear = setup.get("Left Rear").unwrap();
    assert_eq!(left_rear, &expected);

    // Right Front
    let expected = create_ordered_multimap(&[
        ("Cold pressure", "30.0 psi"),
        ("Last hot pressure", "30.0 psi"),
        ("Last temps I M O", "103F"),
        ("Last temps I M O", "103F"),
        ("Last temps I M O", "103F"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
        ("Corner weight", "552 lbs"),
        ("Ride height", "4.84 in"),
        ("Spring perch offset", r#"2.781""#),
        ("Bump stiffness", "+10 clicks"),
        ("Rebound stiffness", "+8 clicks"),
        ("Camber", "-2.7 deg"),
    ]);
    let right_front = setup.get("Right Front").unwrap();
    assert_eq!(right_front, &expected);

    // Right Rear
    let expected = create_ordered_multimap(&[
        ("Cold pressure", "30.0 psi"),
        ("Last hot pressure", "30.0 psi"),
        ("Last temps I M O", "103F"),
        ("Last temps I M O", "103F"),
        ("Last temps I M O", "103F"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
        ("Corner weight", "488 lbs"),
        ("Ride height", "4.87 in"),
        ("Spring perch offset", r#"1.844""#),
        ("Bump stiffness", "+8 clicks"),
        ("Rebound stiffness", "+10 clicks"),
        ("Camber", "-2.7 deg"),
    ]);
    let right_rear = setup.get("Right Rear").unwrap();
    assert_eq!(right_rear, &expected);

    // Rear
    let expected = create_ordered_multimap(&[
        ("Fuel level", "5.3 gal"),
        ("Toe-in", r#"+2/16""#),
        ("Anti-roll bar", "Unhooked"),
    ]);
    let rear = setup.get("Rear").unwrap();
    assert_eq!(rear, &expected);
}

#[test]
fn test_setup_dallara_p217() {
    let config = Config::new("/tmp/some/path.toml", PhysicalSize::new(0, 0));
    let (track_name, car_name, setup) =
        setup_from_html("./fixtures/iracing_lemans_default.htm", &config).unwrap();

    assert_eq!(
        track_name,
        "Circuit des 24 Heures du Mans - 24 Heures du Mans".to_string()
    );
    assert_eq!(car_name, "Dallara P217".to_string());
    assert_eq!(setup.keys().len(), 18);

    // Left Front Tire
    let expected = create_ordered_multimap(&[
        ("Starting pressure", "20.0 psi"),
        ("Last hot pressure", "22.0 psi"),
        ("Last temps O M I", "178F"),
        ("Last temps O M I", "182F"),
        ("Last temps O M I", "187F"),
        ("Tread remaining", "99%"),
        ("Tread remaining", "98%"),
        ("Tread remaining", "98%"),
    ]);
    let left_front_tire = setup.get("Left Front Tire").unwrap();
    assert_eq!(left_front_tire, &expected);

    // Left Rear Tire
    let expected = create_ordered_multimap(&[
        ("Starting pressure", "20.0 psi"),
        ("Last hot pressure", "22.3 psi"),
        ("Last temps O M I", "186F"),
        ("Last temps O M I", "196F"),
        ("Last temps O M I", "200F"),
        ("Tread remaining", "98%"),
        ("Tread remaining", "97%"),
        ("Tread remaining", "97%"),
    ]);
    let left_rear_tire = setup.get("Left Rear Tire").unwrap();
    assert_eq!(left_rear_tire, &expected);

    // Right Front Tire
    let expected = create_ordered_multimap(&[
        ("Starting pressure", "20.0 psi"),
        ("Last hot pressure", "21.8 psi"),
        ("Last temps I M O", "183F"),
        ("Last temps I M O", "179F"),
        ("Last temps I M O", "173F"),
        ("Tread remaining", "98%"),
        ("Tread remaining", "98%"),
        ("Tread remaining", "99%"),
    ]);
    let right_front_tire = setup.get("Right Front Tire").unwrap();
    assert_eq!(right_front_tire, &expected);

    // Right Rear Tire
    let expected = create_ordered_multimap(&[
        ("Starting pressure", "20.0 psi"),
        ("Last hot pressure", "22.1 psi"),
        ("Last temps I M O", "199F"),
        ("Last temps I M O", "195F"),
        ("Last temps I M O", "182F"),
        ("Tread remaining", "97%"),
        ("Tread remaining", "97%"),
        ("Tread remaining", "98%"),
    ]);
    let right_rear_tire = setup.get("Right Rear Tire").unwrap();
    assert_eq!(right_rear_tire, &expected);

    // Aero Settings
    let expected = create_ordered_multimap(&[
        ("Downforce trim", "Low"),
        ("Rear wing angle", "12 deg"),
        ("# of dive planes", "1"),
        ("Wing gurney setting", "Off"),
        ("Deck gurney setting", "On"),
    ]);
    let aero_settings = setup.get("Aero Settings").unwrap();
    assert_eq!(aero_settings, &expected);

    // Aero Calculator
    let expected = create_ordered_multimap(&[
        ("Front RH at speed", r#"1.575""#),
        ("Rear RH at speed", r#"1.181""#),
        ("Downforce balance", "40.48%"),
        ("L/D", "4.981"),
    ]);
    let aero_calculator = setup.get("Aero Calculator").unwrap();
    assert_eq!(aero_calculator, &expected);

    // Front
    let expected = create_ordered_multimap(&[
        ("Third spring", "571 lbs/in"),
        ("Third perch offset", r#"1.791""#),
        ("Third spring defl", "0.292 in"),
        ("Third spring defl", "of"),
        ("Third spring defl", "3.090 in"),
        ("Third slider defl", "1.988 in"),
        ("Third slider defl", "of"),
        ("Third slider defl", "3.937 in"),
        ("ARB size", "Medium"),
        ("ARB blades", "P2"),
        ("Toe-in", r#"-1/32""#),
        ("Third pin length", r#"7.913""#),
        ("Front pushrod length", r#"7.520""#),
        ("Power steering assist", "3"),
        ("Steering ratio", "11.0"),
        ("Display page", "Race1"),
    ]);
    let front = setup.get("Front").unwrap();
    assert_eq!(front, &expected);

    // Left Front
    let expected = create_ordered_multimap(&[
        ("Corner weight", "527 lbs"),
        ("Ride height", "1.772 in"),
        ("Shock defl", "1.070 in"),
        ("Shock defl", "of"),
        ("Shock defl", "1.969 in"),
        ("Torsion bar defl", "0.377 in"),
        ("Torsion bar turns", "5.000 Turns"),
        ("Torsion bar O.D.", "13.90 mm"),
        ("LS comp damping", "2 clicks"),
        ("HS comp damping", "5 clicks"),
        ("HS comp damp slope", "4 clicks"),
        ("LS rbd damping", "4 clicks"),
        ("HS rbd damping", "6 clicks"),
        ("Camber", "-2.8 deg"),
    ]);
    let left_front = setup.get("Left Front").unwrap();
    assert_eq!(left_front, &expected);

    // Left Rear
    let expected = create_ordered_multimap(&[
        ("Corner weight", "652 lbs"),
        ("Ride height", "1.771 in"),
        ("Shock defl", "1.598 in"),
        ("Shock defl", "of"),
        ("Shock defl", "2.953 in"),
        ("Spring defl", "0.547 in"),
        ("Spring defl", "of"),
        ("Spring defl", "3.525 in"),
        ("Spring perch offset", r#"2.146""#),
        ("Spring rate", "600 lbs/in"),
        ("LS comp damping", "2 clicks"),
        ("HS comp damping", "5 clicks"),
        ("HS comp damp slope", "4 clicks"),
        ("LS rbd damping", "4 clicks"),
        ("HS rbd damping", "6 clicks"),
        ("Camber", "-1.8 deg"),
        ("Toe-in", r#"+1/32""#),
    ]);
    let left_rear = setup.get("Left Rear").unwrap();
    assert_eq!(left_rear, &expected);

    // Right Front
    let expected = create_ordered_multimap(&[
        ("Corner weight", "527 lbs"),
        ("Ride height", "1.772 in"),
        ("Shock defl", "1.070 in"),
        ("Shock defl", "of"),
        ("Shock defl", "1.969 in"),
        ("Torsion bar defl", "0.377 in"),
        ("Torsion bar turns", "5.000 Turns"),
        ("Torsion bar O.D.", "13.90 mm"),
        ("LS comp damping", "2 clicks"),
        ("HS comp damping", "5 clicks"),
        ("HS comp damp slope", "4 clicks"),
        ("LS rbd damping", "4 clicks"),
        ("HS rbd damping", "6 clicks"),
        ("Camber", "-2.8 deg"),
    ]);
    let right_front = setup.get("Right Front").unwrap();
    assert_eq!(right_front, &expected);

    // Right Rear
    let expected = create_ordered_multimap(&[
        ("Corner weight", "652 lbs"),
        ("Ride height", "1.771 in"),
        ("Shock defl", "1.598 in"),
        ("Shock defl", "of"),
        ("Shock defl", "2.953 in"),
        ("Spring defl", "0.547 in"),
        ("Spring defl", "of"),
        ("Spring defl", "3.525 in"),
        ("Spring perch offset", r#"2.146""#),
        ("Spring rate", "600 lbs/in"),
        ("LS comp damping", "2 clicks"),
        ("HS comp damping", "5 clicks"),
        ("HS comp damp slope", "4 clicks"),
        ("LS rbd damping", "4 clicks"),
        ("HS rbd damping", "6 clicks"),
        ("Camber", "-1.8 deg"),
        ("Toe-in", r#"+1/32""#),
    ]);
    let right_rear = setup.get("Right Rear").unwrap();
    assert_eq!(right_rear, &expected);

    // Rear
    let expected = create_ordered_multimap(&[
        ("Third spring", "457 lbs/in"),
        ("Third perch offset", r#"1.516""#),
        ("Third spring defl", "0.538 in"),
        ("Third spring defl", "of"),
        ("Third spring defl", "3.753 in"),
        ("Third slider defl", "2.928 in"),
        ("Third slider defl", "of"),
        ("Third slider defl", "5.906 in"),
        ("ARB size", "Medium"),
        ("ARB blades", "P4"),
        ("Rear pushrod length", r#"6.614""#),
        ("Third pin length", r#"7.126""#),
        ("Cross weight", "50.0%"),
    ]);
    let rear = setup.get("Rear").unwrap();
    assert_eq!(rear, &expected);

    // Lighting
    let expected = create_ordered_multimap(&[("Roof ID light color", "Blue")]);
    let lighting = setup.get("Lighting").unwrap();
    assert_eq!(lighting, &expected);

    // Brake Spec
    let expected =
        create_ordered_multimap(&[("Pad compound", "Medium"), ("Brake pressure bias", "49.2%")]);
    let brake_spec = setup.get("Brake Spec").unwrap();
    assert_eq!(brake_spec, &expected);

    // Fuel
    let expected = create_ordered_multimap(&[("Fuel level", "19.8 gal")]);
    let fuel = setup.get("Fuel").unwrap();
    assert_eq!(fuel, &expected);

    // Traction Control
    let expected = create_ordered_multimap(&[
        ("Traction control gain", "5 (TC)"),
        ("Traction control slip", "5 (TC)"),
        ("Throttle shape", "1"),
    ]);
    let traction_control = setup.get("Traction Control").unwrap();
    assert_eq!(traction_control, &expected);

    // Gear Ratios
    let expected = create_ordered_multimap(&[
        ("Gear stack", "Tall"),
        ("Speed in first", "86.7 mph"),
        ("Speed in second", "112.1 mph"),
        ("Speed in third", "131.6 mph"),
        ("Speed in forth", "156.3 mph"),
        ("Speed in fifth", "182.7 mph"),
        ("Speed in sixth", "210.2 mph"),
    ]);
    let gear_ratios = setup.get("Gear Ratios").unwrap();
    assert_eq!(gear_ratios, &expected);

    // Rear Diff Spec
    let expected = create_ordered_multimap(&[
        ("Drive/coast ramp angles", "45/55"),
        ("Clutch friction faces", "4"),
        ("Preload", "55 ft-lbs"),
    ]);
    let rear_diff_spec = setup.get("Rear Diff Spec").unwrap();
    assert_eq!(rear_diff_spec, &expected);
}

#[test]
fn test_setup_porche_911_gt3_r() {
    let config = Config::new("/tmp/some/path.toml", PhysicalSize::new(0, 0));
    let (track_name, car_name, setup) =
        setup_from_html("./fixtures/baseline.htm", &config).unwrap();

    assert_eq!(track_name, "Nürburgring Combined".to_string());
    assert_eq!(car_name, "Porsche 911 GT3 R".to_string());
    assert_eq!(setup.keys().len(), 12);

    // Left Front Tire
    let expected = create_ordered_multimap(&[
        ("Starting pressure", "20.5 psi"),
        ("Last hot pressure", "20.5 psi"),
        ("Last temps O M I", "112F"),
        ("Last temps O M I", "112F"),
        ("Last temps O M I", "112F"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
    ]);
    let left_front_tire = setup.get("Left Front Tire").unwrap();
    assert_eq!(left_front_tire, &expected);

    // Left Rear Tire
    let expected = create_ordered_multimap(&[
        ("Starting pressure", "20.5 psi"),
        ("Last hot pressure", "20.5 psi"),
        ("Last temps O M I", "112F"),
        ("Last temps O M I", "112F"),
        ("Last temps O M I", "112F"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
    ]);
    let left_rear_tire = setup.get("Left Rear Tire").unwrap();
    assert_eq!(left_rear_tire, &expected);

    // Right Front Tire
    let expected = create_ordered_multimap(&[
        ("Starting pressure", "20.5 psi"),
        ("Last hot pressure", "20.5 psi"),
        ("Last temps I M O", "112F"),
        ("Last temps I M O", "112F"),
        ("Last temps I M O", "112F"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
    ]);
    let right_front_tire = setup.get("Right Front Tire").unwrap();
    assert_eq!(right_front_tire, &expected);

    // Right Rear Tire
    let expected = create_ordered_multimap(&[
        ("Starting pressure", "20.5 psi"),
        ("Last hot pressure", "20.5 psi"),
        ("Last temps I M O", "112F"),
        ("Last temps I M O", "112F"),
        ("Last temps I M O", "112F"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
    ]);
    let right_rear_tire = setup.get("Right Rear Tire").unwrap();
    assert_eq!(right_rear_tire, &expected);

    // Aero Balance Calc
    let expected = create_ordered_multimap(&[
        ("Front RH at speed", r#"1.929""#),
        ("Rear RH at speed", r#"2.835""#),
        ("Wing setting", "7 degrees"),
        ("Front downforce", "39.83%"),
    ]);
    let aero_balance_calc = setup.get("Aero Balance Calc").unwrap();
    assert_eq!(aero_balance_calc, &expected);

    // Front
    let expected = create_ordered_multimap(&[
        ("ARB diameter", "45 mm"),
        ("ARB setting", "Soft"),
        ("Toe-in", r#"-2/32""#),
        ("Front master cyl.", "0.811 in"),
        ("Rear master cyl.", "0.811 in"),
        ("Brake pads", "Medium friction"),
        ("Fuel level", "15.9 gal"),
        ("Cross weight", "50.0%"),
    ]);
    let front = setup.get("Front").unwrap();
    assert_eq!(front, &expected);

    // Left Front
    let expected = create_ordered_multimap(&[
        ("Corner weight", "605 lbs"),
        ("Ride height", "2.034 in"),
        ("Spring perch offset", r#"2.441""#),
        ("Spring rate", "1371 lbs/in"),
        ("LS Comp damping", "-6 clicks"),
        ("HS Comp damping", "-10 clicks"),
        ("LS Rbd damping", "-8 clicks"),
        ("HS Rbd damping", "-10 clicks"),
        ("Camber", "-4.0 deg"),
        ("Caster", "+7.6 deg"),
    ]);
    let left_front = setup.get("Left Front").unwrap();
    assert_eq!(left_front, &expected);

    // Left Rear
    let expected = create_ordered_multimap(&[
        ("Corner weight", "945 lbs"),
        ("Ride height", "3.026 in"),
        ("Spring perch offset", r#"2.717""#),
        ("Spring rate", "1600 lbs/in"),
        ("LS Comp damping", "-6 clicks"),
        ("HS Comp damping", "-10 clicks"),
        ("LS Rbd damping", "-8 clicks"),
        ("HS Rbd damping", "-10 clicks"),
        ("Camber", "-3.4 deg"),
        ("Toe-in", r#"+1/64""#),
    ]);
    let left_rear = setup.get("Left Rear").unwrap();
    assert_eq!(left_rear, &expected);

    // In-Car Dials
    let expected = create_ordered_multimap(&[
        ("Display page", "Race 1"),
        ("Brake pressure bias", "54.0%"),
        ("Trac Ctrl (TCC) setting", "5 (TCC)"),
        ("Trac Ctrl (TCR) setting", "5 (TCR)"),
        ("Throttle Map setting", "4"),
        ("ABS setting", "11 (ABS)"),
        ("Engine map setting", "4 (MAP)"),
        ("Night LED strips", "Blue"),
    ]);
    let in_car_dials = setup.get("In-Car Dials").unwrap();
    assert_eq!(in_car_dials, &expected);

    // Right Front
    let expected = create_ordered_multimap(&[
        ("Corner weight", "605 lbs"),
        ("Ride height", "2.034 in"),
        ("Spring perch offset", r#"2.441""#),
        ("Spring rate", "1371 lbs/in"),
        ("LS Comp damping", "-6 clicks"),
        ("HS Comp damping", "-10 clicks"),
        ("LS Rbd damping", "-8 clicks"),
        ("HS Rbd damping", "-10 clicks"),
        ("Camber", "-4.0 deg"),
        ("Caster", "+7.6 deg"),
    ]);
    let right_front = setup.get("Right Front").unwrap();
    assert_eq!(right_front, &expected);

    // Right Rear
    let expected = create_ordered_multimap(&[
        ("Corner weight", "945 lbs"),
        ("Ride height", "3.026 in"),
        ("Spring perch offset", r#"2.717""#),
        ("Spring rate", "1600 lbs/in"),
        ("LS Comp damping", "-6 clicks"),
        ("HS Comp damping", "-10 clicks"),
        ("LS Rbd damping", "-8 clicks"),
        ("HS Rbd damping", "-10 clicks"),
        ("Camber", "-3.4 deg"),
        ("Toe-in", r#"+1/64""#),
    ]);
    let right_rear = setup.get("Right Rear").unwrap();
    assert_eq!(right_rear, &expected);

    // Rear
    let expected = create_ordered_multimap(&[
        ("ARB diameter", "35 mm"),
        ("ARB setting", "Med"),
        ("Diff preload", "74 ft-lbs"),
        ("Wing setting", "7 degrees"),
    ]);
    let rear = setup.get("Rear").unwrap();
    assert_eq!(rear, &expected);
}

#[test]
fn test_setup_mercedes_amg_w12() {
    let config = Config::new("/tmp/some/path.toml", PhysicalSize::new(0, 0));
    let (track_name, car_name, setup) =
        setup_from_html("./fixtures/iracing_w12_baseline_glenboot.htm", &config).unwrap();

    assert_eq!(track_name, "Watkins Glen International".to_string());
    assert_eq!(car_name, "Mercedes-AMG W12 E Performance".to_string());
    assert_eq!(setup.keys().len(), 16);

    dbg!(&setup);

    // Tire Compound
    let expected = create_ordered_multimap(&[("Tire compound", "Medium")]);
    let left_front_tire = setup.get("Tire Compound").unwrap();
    assert_eq!(left_front_tire, &expected);

    // Left Front Tire
    let expected = create_ordered_multimap(&[
        ("Starting pressure", "22.0 psi"),
        ("Last hot pressure", "22.0 psi"),
        ("Last temps O M I", "172F"),
        ("Last temps O M I", "172F"),
        ("Last temps O M I", "172F"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
    ]);
    let left_front_tire = setup.get("Left Front Tire").unwrap();
    assert_eq!(left_front_tire, &expected);

    // Left Rear Tire
    let expected = create_ordered_multimap(&[
        ("Starting pressure", "20.0 psi"),
        ("Last hot pressure", "20.0 psi"),
        ("Last temps O M I", "171F"),
        ("Last temps O M I", "171F"),
        ("Last temps O M I", "171F"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
    ]);
    let left_rear_tire = setup.get("Left Rear Tire").unwrap();
    assert_eq!(left_rear_tire, &expected);

    // Right Front Tire
    let expected = create_ordered_multimap(&[
        ("Starting pressure", "22.0 psi"),
        ("Last hot pressure", "22.0 psi"),
        ("Last temps I M O", "172F"),
        ("Last temps I M O", "172F"),
        ("Last temps I M O", "172F"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
    ]);
    let right_front_tire = setup.get("Right Front Tire").unwrap();
    assert_eq!(right_front_tire, &expected);

    // Right Rear Tire
    let expected = create_ordered_multimap(&[
        ("Starting pressure", "20.0 psi"),
        ("Last hot pressure", "20.0 psi"),
        ("Last temps I M O", "171F"),
        ("Last temps I M O", "171F"),
        ("Last temps I M O", "171F"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
        ("Tread remaining", "100%"),
    ]);
    let right_rear_tire = setup.get("Right Rear Tire").unwrap();
    assert_eq!(right_rear_tire, &expected);

    // Aero Package
    let expected = create_ordered_multimap(&[
        ("Downforce trim", "High"),
        ("Front flap offset", "1.75 deg"),
        ("Rear wing Gurney", "0.591 in"),
    ]);
    let aero_package = setup.get("Aero Package").unwrap();
    assert_eq!(aero_package, &expected);

    // Aero Calculator
    let expected = create_ordered_multimap(&[
        ("Front RH at speed", r#"0.512""#),
        ("Rear RH at speed", r#"2.835""#),
        ("Aero balance", "44.80%"),
        ("Downforce to drag", "4.393:1"),
    ]);
    let aero_calculator = setup.get("Aero Calculator").unwrap();
    assert_eq!(aero_calculator, &expected);

    // Front
    let expected = create_ordered_multimap(&[
        ("Transparent halo", "No"),
        ("Weight dist", "48.0%"),
        ("Heave rate", "4000 lbs/in"),
        ("Roll rate", "2284 lb/in"),
        ("Ride height", "0.950 in"),
    ]);
    let front = setup.get("Front").unwrap();
    assert_eq!(front, &expected);

    // Left Front
    let expected = create_ordered_multimap(&[
        ("Corner weight", "499 lbs"),
        ("Camber", "-3.48 deg"),
        ("Toe-in", "-0.24 deg"),
    ]);
    let left_front = setup.get("Left Front").unwrap();
    assert_eq!(left_front, &expected);

    // Left Rear
    let expected = create_ordered_multimap(&[
        ("Corner weight", "540 lbs"),
        ("Camber", "-1.95 deg"),
        ("Toe-in", "+0.10 deg"),
    ]);
    let left_rear = setup.get("Left Rear").unwrap();
    assert_eq!(left_rear, &expected);

    // Right Front
    let expected = create_ordered_multimap(&[
        ("Corner weight", "499 lbs"),
        ("Camber", "-3.48 deg"),
        ("Toe-in", "-0.24 deg"),
    ]);
    let right_front = setup.get("Right Front").unwrap();
    assert_eq!(right_front, &expected);

    // Right Rear
    let expected = create_ordered_multimap(&[
        ("Corner weight", "540 lbs"),
        ("Camber", "-1.95 deg"),
        ("Toe-in", "+0.10 deg"),
    ]);
    let right_rear = setup.get("Right Rear").unwrap();
    assert_eq!(right_rear, &expected);

    // Rear
    let expected = create_ordered_multimap(&[
        ("Fuel level", "242.5 lb"),
        ("Heave rate", "286 lbs/in"),
        ("Roll rate", "1199 lb/in"),
        ("Ride height", "5.442 in"),
    ]);
    let rear = setup.get("Rear").unwrap();
    assert_eq!(rear, &expected);

    // Differential
    let expected = create_ordered_multimap(&[
        ("Preload", "0 ft-lbs"),
        ("Entry", "1 (ENTRY)"),
        ("Middle", "2 (MID)"),
        ("High speed", "3 (HISPD)"),
    ]);
    let differential = setup.get("Differential").unwrap();
    assert_eq!(differential, &expected);

    // Power Unit Config
    let expected = create_ordered_multimap(&[
        ("MGU-K deploy mode", "Balanced"),
        ("Engine braking", "10 (EB)"),
    ]);
    let power_unit_config = setup.get("Power Unit Config").unwrap();
    assert_eq!(power_unit_config, &expected);

    // Brake System Config
    let expected = create_ordered_multimap(&[
        ("Base brake bias", "57.0% (BBAL)"),
        ("Dynamic ramping", "50% pedal"),
        ("Brake migration", "1 (BMIG)"),
        ("Total brake bias", "57.0% front"),
        ("Brake magic modifier", "0.75"),
    ]);
    let brake_system_config = setup.get("Brake System Config").unwrap();
    assert_eq!(brake_system_config, &expected);
}

#[test]
fn test_add_setup() {
    use UpdateKind::*;

    fn assert_added(setups: &Setups, file_name: &str) {
        let tracks = setups.tracks();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks["Nürburgring Combined"].len(), 1);
        assert_eq!(tracks["Nürburgring Combined"]["Porsche 911 GT3 R"].len(), 1);

        let SetupInfo { setup, name, .. } = &tracks["Nürburgring Combined"]["Porsche 911 GT3 R"][0];

        assert_eq!(name, file_name);
        assert_eq!(setup.keys().len(), 12);
    }

    let mut setups = Setups::default();
    assert!(setups.tracks.is_empty());

    let config = Config::new("/tmp/some/path.toml", PhysicalSize::new(0, 0));
    let mut result = Vec::new();
    let path = Path::new("./fixtures/baseline.htm")
        .canonicalize()
        .expect("Cannot canonicalize path");

    // Test adding a setup to an empty tree
    setups.add(&mut result, &path, None, &config);

    assert_eq!(
        &result,
        &[AddedSetup(
            "Nürburgring Combined".to_string(),
            "Porsche 911 GT3 R".to_string(),
            0
        )]
    );
    assert_added(&setups, "baseline");

    // Test adding an existing setup to the tree
    result.clear();
    setups.add(&mut result, &path, None, &config);

    assert_eq!(&result, &[]);
    assert_added(&setups, "baseline");
}

#[test]
fn test_remove_setup() {
    use UpdateKind::*;

    fn assert_removed(setups: &Setups) {
        let tracks = setups.tracks();
        assert_eq!(tracks.len(), 4);
        assert!(tracks.contains_key("Centripetal Circuit"));
        assert!(tracks.contains_key("Charlotte Motor Speedway"));
        assert!(tracks.contains_key("Circuit des 24 Heures du Mans - 24 Heures du Mans"));
        assert!(tracks.contains_key("Watkins Glen International"));
    }

    let mut config = Config::new("/tmp/some/path.toml", PhysicalSize::new(0, 0));
    config.update_setups_path("./fixtures");
    let mut warnings = VecDeque::new();
    let mut setups = Setups::new(&mut warnings, &config);

    let tracks = setups.tracks();
    assert_eq!(tracks.len(), 5);
    assert!(tracks.contains_key("Centripetal Circuit"));
    assert!(tracks.contains_key("Charlotte Motor Speedway"));
    assert!(tracks.contains_key("Circuit des 24 Heures du Mans - 24 Heures du Mans"));
    assert!(tracks.contains_key("Nürburgring Combined"));
    assert!(tracks.contains_key("Watkins Glen International"));

    let mut result = Vec::new();
    let path = Path::new("./fixtures/baseline.htm")
        .canonicalize()
        .expect("Cannot canonicalize path");

    // Test removing a setup from the tree
    setups.remove(&mut result, &path);

    assert_eq!(
        &result,
        &[
            RemovedSetup(
                "Nürburgring Combined".to_string(),
                "Porsche 911 GT3 R".to_string(),
                0
            ),
            RemovedCar(
                "Nürburgring Combined".to_string(),
                "Porsche 911 GT3 R".to_string()
            ),
            RemovedTrack("Nürburgring Combined".to_string()),
        ]
    );
    assert_removed(&setups);

    // Test removing a non-existent setup from the tree
    result.clear();
    setups.remove(&mut result, &path);

    assert_eq!(&result, &[]);
    assert_removed(&setups);
}

#[test]
fn test_update_setup() {
    use UpdateKind::*;

    let mut setups = Setups::default();
    assert!(setups.tracks.is_empty());

    let config = Config::new("/tmp/some/path.toml", PhysicalSize::new(0, 0));
    let path1 = Path::new("./fixtures/baseline.htm")
        .canonicalize()
        .expect("Cannot canonicalize path");
    let path2 = Path::new("./fixtures/skip_barber_centripetal.htm")
        .canonicalize()
        .expect("Cannot canonicalize path");
    let path3 = tempfile::Builder::new()
        .suffix(".html")
        .tempfile()
        .expect("Unable to create temp file")
        .path()
        .canonicalize()
        .expect("Cannot canonicalize path");
    std::fs::copy(&path2, &path3).expect("Unable to copy file");
    let path4 = tempfile::Builder::new()
        .suffix(".non-html-file")
        .tempfile()
        .expect("Unable to create temp file")
        .path()
        .canonicalize()
        .expect("Cannot canonicalize path");

    // Test adding a setup to an empty tree with Write
    let event = hotwatch::Event::Create(path1.clone());
    let result = setups.update(&event, &config);

    assert_eq!(
        &result,
        &[AddedSetup(
            "Nürburgring Combined".to_string(),
            "Porsche 911 GT3 R".to_string(),
            0
        )]
    );
    assert_eq!(setups.tracks.len(), 1);

    // Test adding an existing setup to the tree
    let event = hotwatch::Event::Write(path1.clone());
    let result = setups.update(&event, &config);

    assert_eq!(&result, &[]);
    assert_eq!(setups.tracks.len(), 1);

    // Test adding a setup to the tree with Create
    let event = hotwatch::Event::Create(path2.clone());
    let result = setups.update(&event, &config);

    assert_eq!(
        &result,
        &[AddedSetup(
            "Centripetal Circuit".to_string(),
            "Skip Barber Formula 2000".to_string(),
            0
        )]
    );
    assert_eq!(setups.tracks.len(), 2);

    // Test removing a setup from the tree
    let event = hotwatch::Event::Remove(path1);
    let result = setups.update(&event, &config);

    assert_eq!(
        &result,
        &[
            RemovedSetup(
                "Nürburgring Combined".to_string(),
                "Porsche 911 GT3 R".to_string(),
                0
            ),
            RemovedCar(
                "Nürburgring Combined".to_string(),
                "Porsche 911 GT3 R".to_string()
            ),
            RemovedTrack("Nürburgring Combined".to_string()),
        ]
    );
    assert_eq!(setups.tracks.len(), 1);

    // Test renaming a setup in the tree
    let name = &setups.tracks["Centripetal Circuit"]["Skip Barber Formula 2000"][0].name;
    assert_eq!(name, "skip_barber_centripetal");

    let event = hotwatch::Event::Rename(path2, path3.clone());
    let result = setups.update(&event, &config);

    assert_eq!(&result, &[]);
    assert_eq!(setups.tracks.len(), 1);

    let name = &setups.tracks["Centripetal Circuit"]["Skip Barber Formula 2000"][0].name;
    let expected_name = path3
        .as_path()
        .file_stem()
        .expect("Unable to get file stem")
        .to_str()
        .expect("Unable to convert &OsStr to &str");
    assert_eq!(name, expected_name);

    // Test renaming a setup in the tree to a non-html (unparseable) file
    let event = hotwatch::Event::Rename(path3, path4);
    let result = setups.update(&event, &config);

    assert_eq!(
        &result,
        &[
            RemovedSetup(
                "Centripetal Circuit".to_string(),
                "Skip Barber Formula 2000".to_string(),
                0
            ),
            RemovedCar(
                "Centripetal Circuit".to_string(),
                "Skip Barber Formula 2000".to_string()
            ),
            RemovedTrack("Centripetal Circuit".to_string()),
        ]
    );
    assert!(setups.tracks.is_empty());
}
