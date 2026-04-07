fn has_left_options(s: &String) -> bool {
    for (i, c) in s.chars().enumerate() {
        if c == 'T' {
            if let Some(c1) = s.chars().nth(i + 1) {
                if c1 == ' ' {
                    return true;
                }
                if let Some(c2) = s.chars().nth(i + 2) {
                    if c1 == 'F' && c2 == ' ' {
                        return true;
                    }
                }
            }
        }
    }
    return false;
}

fn get_left_options(s: &String) -> Vec<String> {
    let mut options: Vec<String> = Vec::new();

    for (i, c) in s.chars().enumerate() {
        if c == 'T' {
            if let Some(c1) = s.chars().nth(i + 1) {
                if c1 == ' ' {
                    let mut temp_str = s.clone();
                    temp_str = temp_str
                        .chars()
                        .enumerate()
                        .map(|(j, ch)| {
                            if j == i {
                                ' '
                            } else if j == i + 1 {
                                'T'
                            } else {
                                ch
                            }
                        })
                        .collect();
                    options.push(temp_str);
                }
                if let Some(c2) = s.chars().nth(i + 2) {
                    if c1 == 'F' && c2 == ' ' {
                        let mut temp_str = s.clone();
                        temp_str = temp_str
                            .chars()
                            .enumerate()
                            .map(|(j, ch)| {
                                if j == i {
                                    ' '
                                } else if j == i + 2 {
                                    'T'
                                } else {
                                    ch
                                }
                            })
                            .collect();
                        options.push(temp_str);
                    }
                }
            }
        }
    }
    return options;
}

fn main() {
    let options_game_strings = [String::from("TT FF"), String::from(""), String::from("TF ")];
    let moves_game_strings = [
        String::from("TT FF"),
        String::from("T TF TF"),
        String::from(""),
        String::from("TF "),
    ];

    println!("Has Options Tests");
    for gs in &options_game_strings {
        let result = has_left_options(gs);
        println!("Game String: {} Results: {}", gs, result);
    }

    println!();
    println!("Possible Moves Tests");
    for gs in &moves_game_strings {
        let result = get_left_options(gs);
        println!("Game String: {} Results: {:?}", gs, result);
    }
}
