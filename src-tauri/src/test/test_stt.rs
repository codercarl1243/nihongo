use nihongo_lib::stt::transcribe;

fn main() {
    let result = transcribe::transcribe_audio("/Users/carl/projects/codercarl/nihongo/test-pipeline/test.wav");
    let result2 = transcribe::transcribe_audio("/Users/carl/Library/Application Support/com.carl.nihongo/voice-recordings/recording-1777526690597.wav");


    match result {
        Ok(text) => println!("TRANSCRIBED: {}", text),
        Err(e) => eprintln!("ERROR: {}", e),
    }
}