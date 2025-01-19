use std::net::UdpSocket;
use std::time::{Instant, Duration};
use std::io::ErrorKind;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Move {
    Rock,
    Paper,
    Scissors,
}

impl Move {
    fn from_char(c: char) -> Option<Move> {
        match c {
            'r' | 'R' => Some(Move::Rock),
            'p' | 'P' => Some(Move::Paper),
            's' | 'S' => Some(Move::Scissors),
            _ => None,
        }
    }

    /// Returns:
    ///  1 if self beats other,
    /// -1 if other beats self,
    ///  0 if tie
    fn compare(&self, other: Move) -> i32 {
        use Move::*;
        match (*self, other) {
            (Rock, Scissors) | (Scissors, Paper) | (Paper, Rock) => 1,
            (Scissors, Rock) | (Paper, Scissors) | (Rock, Paper) => -1,
            _ => 0,
        }
    }
}

impl std::fmt::Display for Move {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Move::Rock => write!(f, "Rock"),
            Move::Paper => write!(f, "Paper"),
            Move::Scissors => write!(f, "Scissors"),
        }
    }
}

fn main() -> std::io::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:5000")?;
    socket.set_nonblocking(true)?;

    // Keep track of player addresses
    let mut player1_addr: Option<std::net::SocketAddr> = None;
    let mut player2_addr: Option<std::net::SocketAddr> = None;

    println!("Server listening on 0.0.0.0:5000... Waiting for players to connect.");

    // 1) WAITING LOOP
    //    We loop here until we have two unique players.
    loop {
        // We'll poll for incoming messages
        let mut buf = [0u8; 128];
        match socket.recv_from(&mut buf) {
            Ok((_size, src_addr)) => {
                // If no player1 yet, assign this one as player1
                if player1_addr.is_none() {
                    player1_addr = Some(src_addr);
                    println!("Player 1 connected: {}", src_addr);
                } 
                // Else if no player2 yet (and it's not the same as player1),
                // assign this one as player2
                else if player2_addr.is_none() && Some(src_addr) != player1_addr {
                    player2_addr = Some(src_addr);
                    println!("Player 2 connected: {}", src_addr);
                }

                // If both are assigned, break out of waiting loop
                if player1_addr.is_some() && player2_addr.is_some() {
                    println!("Both players connected! Starting the game...");
                    break;
                }
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                // No data waiting, so just continue the loop
            }
            Err(e) => {
                eprintln!("Error receiving data: {:?}", e);
            }
        }

        // You could sleep a tiny bit here to avoid spinning 100% CPU, if desired.
        // std::thread::sleep(Duration::from_millis(1));
    }

    // 2) GAME LOOP
    //    Now that we have two players, we do the timed RPS logic.
    let mut player1_move = Move::Rock;
    let mut player2_move = Move::Rock;
    let mut player1_score = 0;
    let mut player2_score = 0;

    // If you want a fast tick (e.g. 500 microseconds),
    // you can keep it, but let's do 1 second for demonstration:
    let tick_duration = Duration::from_micros(500);
    let mut last_tick = Instant::now();

    // 60-second game
    let total_game_duration = Duration::from_secs(20);
    let start_time = Instant::now();

    loop {
        let elapsed = start_time.elapsed();
        if elapsed >= total_game_duration {
            // Time's up
            let result_msg = if player1_score > player2_score {
                "Player 1 wins!"
            } else if player2_score > player1_score {
                "Player 2 wins!"
            } else {
                "It's a draw!"
            };

            broadcast_state(
                &socket,
                player1_addr,
                player2_addr,
                player1_move,
                player2_move,
                player1_score,
                player2_score,
                0,
                Some(result_msg),
            );
            println!("Game ended: {}", result_msg);
            break;
        }

        // Check for incoming messages
        let mut buf = [0u8; 128];
        match socket.recv_from(&mut buf) {
            Ok((size, src_addr)) => {
                let msg = String::from_utf8_lossy(&buf[..size]).trim().to_string();

                if Some(src_addr) == player1_addr {
                    // Player 1's move
                    if let Some(new_move) = msg.chars().next().and_then(Move::from_char) {
                        player1_move = new_move;
                        println!("Player 1 chose {:?}", new_move);
                    }
                } else if Some(src_addr) == player2_addr {
                    // Player 2's move
                    if let Some(new_move) = msg.chars().next().and_then(Move::from_char) {
                        player2_move = new_move;
                        println!("Player 2 chose {:?}", new_move);
                    }
                }
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                // No data, continue
            }
            Err(e) => {
                eprintln!("Error receiving data: {:?}", e);
            }
        }

        // Time to tick?
        if last_tick.elapsed() >= tick_duration {
            last_tick = Instant::now();
            let cmp = player1_move.compare(player2_move);
            if cmp == 1 {
                player1_score += 1;
            } else if cmp == -1 {
                player2_score += 1;
            }

            // Broadcast
            let time_left = (total_game_duration - elapsed).as_secs();
            broadcast_state(
                &socket,
                player1_addr,
                player2_addr,
                player1_move,
                player2_move,
                player1_score,
                player2_score,
                time_left,
                None,
            );
        }
    }

    Ok(())
}

fn broadcast_state(
    socket: &UdpSocket,
    p1_addr: Option<std::net::SocketAddr>,
    p2_addr: Option<std::net::SocketAddr>,
    p1_move: Move,
    p2_move: Move,
    p1_score: u32,
    p2_score: u32,
    time_left: u64,
    final_msg: Option<&str>,
) {
    use std::fmt::Write; // for write! macro in a String

    let mut msg = String::new();
    write!(
        msg,
        "Player 1: {}, Player 2: {}, SCORES => Player 1: {}, Player 2: {} | Time Left: {} sec",
        p1_move, p2_move, p1_score, p2_score, time_left
    ).unwrap();

    if let Some(m) = final_msg {
        write!(msg, " | {}", m).unwrap();
    }

    if let Some(addr) = p1_addr {
        let _ = socket.send_to(msg.as_bytes(), addr);
    }
    if let Some(addr) = p2_addr {
        let _ = socket.send_to(msg.as_bytes(), addr);
    }
}
