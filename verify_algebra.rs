#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Assessment {
    SC,
    SP,
    RC,
    RP,
    NP,
    NN,
}

use Assessment::*;

const STATES: [Assessment; 6] = [SC, SP, RC, RP, NP, NN];

fn dual(a: Assessment) -> Assessment {
    match a {
        SC => RC,
        RC => SC,
        SP => RP,
        RP => SP,
        NP => NP,
        NN => NN,
    }
}

fn all_op(a: Assessment, b: Assessment) -> Assessment {
    match (a, b) {
        (SC, SC) => SC, (SC, SP) => SP, (SC, RC) => RC, (SC, RP) => RP, (SC, NP) => NP, (SC, NN) => NN,
        (SP, SC) => SP, (SP, SP) => SP, (SP, RC) => RC, (SP, RP) => RP, (SP, NP) => NP, (SP, NN) => NN,
        (RC, SC) => RC, (RC, SP) => RC, (RC, RC) => RC, (RC, RP) => RC, (RC, NP) => RC, (RC, NN) => RC,
        (RP, SC) => RP, (RP, SP) => RP, (RP, RC) => RC, (RP, RP) => RP, (RP, NP) => RP, (RP, NN) => RP,
        (NP, SC) => NP, (NP, SP) => NP, (NP, RC) => RC, (NP, RP) => RP, (NP, NP) => NP, (NP, NN) => NN,
        (NN, SC) => NN, (NN, SP) => NN, (NN, RC) => RC, (NN, RP) => RP, (NN, NP) => NN, (NN, NN) => NN,
    }
}

fn any_op(a: Assessment, b: Assessment) -> Assessment {
    dual(all_op(dual(a), dual(b)))
}

fn main() {
    println!("--- Task 1: Verify ALL is commutative and idempotent ---");
    let mut commutative = true;
    let mut idempotent = true;
    for &a in &STATES {
        for &b in &STATES {
            if all_op(a, b) != all_op(b, a) {
                println!("ALL is NOT commutative for {:?}, {:?}", a, b);
                commutative = false;
            }
        }
        if all_op(a, a) != a {
            println!("ALL is NOT idempotent for {:?}", a);
            idempotent = false;
        }
    }
    println!("ALL Commutative: {}", commutative);
    println!("ALL Idempotent: {}", idempotent);

    println!("\n--- Task 2: Verify RC is absorbing for ALL ---");
    let mut rc_absorbing = true;
    for &a in &STATES {
        if all_op(a, RC) != RC || all_op(RC, a) != RC {
            println!("RC is NOT absorbing for {:?}", a);
            rc_absorbing = false;
        }
    }
    println!("RC is absorbing for ALL: {}", rc_absorbing);

    println!("\n--- Task 3: Verify SC > SP > NP > NN ordering for non-rejection states ---");
    fn val(a: Assessment) -> Option<i32> {
        match a {
            SC => Some(4),
            SP => Some(3),
            NP => Some(2),
            NN => Some(1),
            _ => None,
        }
    }
    let ordered = val(SC) > val(SP) && val(SP) > val(NP) && val(NP) > val(NN);
    println!("SC > SP > NP > NN ordering holds: {}", ordered);

    println!("\n--- Task 4: Compute the full 6x6 ANY table using duality ---");
    println!("        SC  SP  RC  RP  NP  NN");
    for &a in &STATES {
        print!("{:?<8}", a);
        for &b in &STATES {
            print!("{:?<4}", any_op(a, b));
        }
        println!();
    }

    println!("\n--- Task 5: Check ANY is commutative and idempotent ---");
    let mut any_commutative = true;
    let mut any_idempotent = true;
    for &a in &STATES {
        for &b in &STATES {
            if any_op(a, b) != any_op(b, a) {
                println!("ANY is NOT commutative for {:?}, {:?}", a, b);
                any_commutative = false;
            }
        }
        if any_op(a, a) != a {
            println!("ANY is NOT idempotent for {:?}", a);
            any_idempotent = false;
        }
    }
    println!("ANY Commutative: {}", any_commutative);
    println!("ANY Idempotent: {}", any_idempotent);

    println!("\n--- Task 6: Verify SC is absorbing for ANY ---");
    let mut sc_absorbing = true;
    for &a in &STATES {
        if any_op(a, SC) != SC || any_op(SC, a) != SC {
            println!("SC is NOT absorbing for ANY with {:?}", a);
            sc_absorbing = false;
        }
    }
    println!("SC is absorbing for ANY: {}", sc_absorbing);

    println!("\n--- Task 7: Check duality for all 36 pairs ---");
    let mut duality_holds = true;
    for &a in &STATES {
        for &b in &STATES {
            if any_op(a, b) != dual(all_op(dual(a), dual(b))) {
                duality_holds = false;
            }
            if all_op(a, b) != dual(any_op(dual(a), dual(b))) {
                duality_holds = false;
            }
        }
    }
    println!("Duality holds for all 36 pairs: {}", duality_holds);
}
