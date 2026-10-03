use thysalion_test_support::replay::InputRecord;

fn assert_input_record_is_uninhabited(record: InputRecord) -> ! {
    match record {}
}

fn main() {
    let _assertion: fn(InputRecord) -> ! = assert_input_record_is_uninhabited;
}
