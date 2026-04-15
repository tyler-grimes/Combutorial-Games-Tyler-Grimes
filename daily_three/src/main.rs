fn has_right_options(s: &String) -> bool {
    let chars: Vec<char> = s.chars().collect();

    for i in (0..chars.len()).rev() {
        if chars[i] == 'F' {
            if i >= 1 && chars[i - 1] == ' ' {
                return true;
            }
            if i >= 2 && chars[i - 1] == 'T' && chars[i - 2] == ' ' {
                return true;
            }
        }
    }
    false
}

fn get_right_options(s: &String) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    let mut moves: Vec<String> = Vec::new();

    for i in (0..chars.len()).rev() {
        if chars[i] == 'F' {
            if i >= 1 && chars[i - 1] == ' ' {
                let mut mut_s: Vec<char> = chars.clone();
                mut_s[i] = ' ';
                mut_s[i - 1] = 'F';
                moves.push(mut_s.iter().collect());
            }
            if i >= 2 && chars[i - 1] == 'T' && chars[i - 2] == ' ' {
                let mut mut_s: Vec<char> = chars.clone();
                mut_s[i] = ' ';
                mut_s[i - 2] = 'F';
                moves.push(mut_s.iter().collect());
            }
        }
    }
    return moves;
}

fn main() {
    // --- has_right_options tests ---

    // Test 1: "F" at index 0, no valid moves
    let s = String::from("F");
    println!(
        "has Test 1 - F at index 0:    {}",
        if !has_right_options(&s) {
            "PASS"
        } else {
            "FAIL"
        }
    );

    // Test 2: " F" -> space before F, valid move exists
    let s = String::from(" F");
    println!(
        "has Test 2 - space then F:    {}",
        if has_right_options(&s) {
            "PASS"
        } else {
            "FAIL"
        }
    );

    // Test 3: " TF" -> space before TF, valid jump exists
    let s = String::from(" TF");
    println!(
        "has Test 3 - space then TF:   {}",
        if has_right_options(&s) {
            "PASS"
        } else {
            "FAIL"
        }
    );

    // --- get_right_options tests ---

    // Test 4: " F" -> one move, F shifts left to "F "
    let s = String::from(" F");
    let result = get_right_options(&s);
    let pass = result.len() == 1 && result[0] == "F ";
    println!(
        "get Test 4 - space then F:    {}",
        if pass { "PASS" } else { "FAIL" }
    );

    // Test 5: " TF" -> one move, F jumps over T to "F T"
    let s = String::from(" TF");
    let result = get_right_options(&s);
    let pass = result.len() == 1 && result[0] == "FT ";
    println!(
        "get Test 5 - space then TF:   {}",
        if pass { "PASS" } else { "FAIL" }
    );

    // Test 6: "F" -> no valid moves, empty vec
    let s = String::from("F");
    let result = get_right_options(&s);
    println!(
        "get Test 6 - no valid moves:  {}",
        if result.is_empty() { "PASS" } else { "FAIL" }
    );
}
