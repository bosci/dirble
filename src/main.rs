use std::fs::File;
use std::io::{self, BufRead, BufReader};
extern crate curl;
use curl::easy::{Easy2, Handler, WriteError};
use percent_encoding::{utf8_percent_encode, DEFAULT_ENCODE_SET};
#[macro_use]
extern crate clap;
use clap::App;
use std::sync::{mpsc, Arc};
use std::path::Path;
use std::thread;


struct Collector(Vec<u8>);

impl Handler for Collector {
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        self.0 = data.to_vec();
        Ok(data.len())
    }
}


fn main() {
    // Load the yaml file containing argument definitions
    let yml = load_yaml!("args.yml");
    let m = App::from_yaml(yml).get_matches();

    // Get the wordlist file from the arguments and open it
    let wordlist = Arc::new(lines_from_file(m.value_of("wordlist").unwrap()).unwrap());
    //let file = File::open(m.value_of("wordlist").unwrap()).unwrap();

    // Get the host URI from the arguments
    let hostname = Arc::new(String::from(m.value_of("host").unwrap().clone()));
    assert!(hostname.starts_with("https://") || hostname.starts_with("http://"), 
        "The provided target URI must start with http:// or https://");

    let (tx, rx) = mpsc::channel();
    let mut handles = vec![];

    let tx_clone = mpsc::Sender::clone(&tx);
    let wordlist_clone = wordlist.clone();
    let hostname_clone = hostname.clone();
    let handle = thread::spawn(|| 
        thread_spawn(tx_clone, hostname_clone, wordlist_clone));

    handles.push(handle);

    for handle in handles {
        handle.join().unwrap();
    }

}

// This function takes an instance of "Easy2", a base URL and a suffix
// It then makes the request, if the response was not a 404
// then it will print the URI it requested and the response
fn request(easy: &mut Easy2<Collector>, base: &str, end: &str) -> u32{

    //Concatenate and url encode the url, then set it in the Easy2 instance
    let url = format!("{}/{}", base, end);
    let url = utf8_percent_encode(&url, DEFAULT_ENCODE_SET).to_string();
    easy.url(&url).unwrap();

    //Perform the request and check if it's empty
    //If it's empty then output info and return
    match easy.perform() {
        Ok(_v) => {}
        Err(_e) => {   println!("- {} (CODE: 0|SIZE: 0)", url);
            return 0; 
        }
    }

    // Get the response code
    let code = easy.response_code().unwrap();
    // Get the contents of the response
    let contents = easy.get_ref();
    
    // Print some output if the 
    match code {
        404 => return code,
        301 | 302 => {
            let content_len = String::from_utf8_lossy(&contents.0).len();
            let redir_dest = easy.redirect_url().unwrap().unwrap();
            let dir_url = url.clone() + "/";
            if dir_url == redir_dest {
                println!("==> DIRECTORY: {}", dir_url);
                1
            }
            else {
                println!("+ {} (CODE: {}|SIZE:{:#?}|DEST:{})", url, code, content_len, redir_dest);
                code
            }
        },
        _ => {
            let content_len = String::from_utf8_lossy(&contents.0).len();
            println!("+ {} (CODE:{}|SIZE:{:#?})", url, code, content_len); 
            code
        },
    }
}

fn thread_spawn(tx: mpsc::Sender<String>, hostname: Arc<String>, wordlist: Arc<Vec<String>>) {
    // Create a new curl Easy2 instance and set it to use GET requests
    let mut easy = Easy2::new(Collector(Vec::new()));
    easy.get(true).unwrap();

    // For each line in the file, call the request function on it
    for line in 0..wordlist.len() {
        //let line ;
        let code = request(&mut easy, &hostname, &wordlist[line]);
        match code {
            1 => {
                let dir_url = format!("{}/", &wordlist[line]);
                tx.send(dir_url).unwrap();
            },
            _ => {},
        }
    }

    tx.send(String::from("END")).unwrap();
}

fn lines_from_file<P>(filename: P) -> io::Result<Vec<String>>
where
    P: AsRef<Path>,
{
    BufReader::new(File::open(filename)?).lines().collect()
}