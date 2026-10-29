use nexus_evidence_requirements::Assessment;

const STATES: [Assessment; 6] = [
    Assessment::SC,
    Assessment::SP,
    Assessment::RC,
    Assessment::RP,
    Assessment::NP,
    Assessment::NN,
];

#[test]
fn dual_table_is_exact() {
    let expected = [
        Assessment::RC,
        Assessment::RP,
        Assessment::SC,
        Assessment::SP,
        Assessment::NP,
        Assessment::NN,
    ];

    for (state, expected_dual) in STATES.into_iter().zip(expected) {
        assert_eq!(state.dual(), expected_dual);
    }
}

#[test]
fn all_table_is_exact() {
    use Assessment::*;

    let expected = [
        [SC, SP, RC, RP, NP, NN],
        [SP, SP, RC, RP, NP, NN],
        [RC, RC, RC, RC, RC, RC],
        [RP, RP, RC, RP, RP, RP],
        [NP, NP, RC, RP, NP, NN],
        [NN, NN, RC, RP, NN, NN],
    ];

    for (i, left) in STATES.into_iter().enumerate() {
        for (j, right) in STATES.into_iter().enumerate() {
            assert_eq!(left.all(right), expected[i][j], "ALL({left:?}, {right:?})");
        }
    }
}

#[test]
fn any_table_is_exact() {
    use Assessment::*;

    let expected = [
        [SC, SC, SC, SC, SC, SC],
        [SC, SP, SP, SP, SP, SP],
        [SC, SP, RC, RP, NP, NN],
        [SC, SP, RP, RP, NP, NN],
        [SC, SP, NP, NP, NP, NN],
        [SC, SP, NN, NN, NN, NN],
    ];

    for (i, left) in STATES.into_iter().enumerate() {
        for (j, right) in STATES.into_iter().enumerate() {
            assert_eq!(left.any(right), expected[i][j], "ANY({left:?}, {right:?})");
        }
    }
}

#[test]
fn all_is_closed() {
    for left in STATES {
        for right in STATES {
            assert!(STATES.contains(&left.all(right)));
        }
    }
}

#[test]
fn any_is_closed() {
    for left in STATES {
        for right in STATES {
            assert!(STATES.contains(&left.any(right)));
        }
    }
}

#[test]
fn all_is_commutative() {
    for left in STATES {
        for right in STATES {
            assert_eq!(left.all(right), right.all(left));
        }
    }
}

#[test]
fn any_is_commutative() {
    for left in STATES {
        for right in STATES {
            assert_eq!(left.any(right), right.any(left));
        }
    }
}

#[test]
fn all_is_idempotent() {
    for state in STATES {
        assert_eq!(state.all(state), state);
    }
}

#[test]
fn any_is_idempotent() {
    for state in STATES {
        assert_eq!(state.any(state), state);
    }
}

#[test]
fn rc_absorbs_all() {
    for state in STATES {
        assert_eq!(Assessment::RC.all(state), Assessment::RC);
        assert_eq!(state.all(Assessment::RC), Assessment::RC);
    }
}

#[test]
fn sc_absorbs_any() {
    for state in STATES {
        assert_eq!(Assessment::SC.any(state), Assessment::SC);
        assert_eq!(state.any(Assessment::SC), Assessment::SC);
    }
}

#[test]
fn any_is_derived_from_all_by_duality() {
    for left in STATES {
        for right in STATES {
            let expected = left.dual().all(right.dual()).dual();
            assert_eq!(left.any(right), expected);
        }
    }
}

#[test]
fn full_duality_holds_for_all_36_pairs() {
    for left in STATES {
        for right in STATES {
            let lhs = left.any(right);
            let rhs = left.dual().all(right.dual()).dual();
            assert_eq!(lhs, rhs, "duality failed for {left:?}, {right:?}");
        }
    }
}
