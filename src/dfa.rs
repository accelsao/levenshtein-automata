use super::Distance;
use std::collections::HashMap;

fn fill(dest: &mut [u32], val: u32) {
    for d in dest {
        *d = val;
    }
}


/// Implementation of a Deterministic Finite Automaton for
/// a Levenshtein Automaton targeting UTF-8 encoded strings.
pub struct DFA {
    transitions: Vec<[u32; 256]>,
    distances: Vec<Distance>,
    initial_state: u32,
}

impl DFA {

    /// Returns the initial state
    ///
    /// The initial state can be anything.
    pub fn initial_state(&self) -> u32 {
        self.initial_state
    }

    /// Returns the resulting distance of
    pub fn eval<B: AsRef<[u8]>>(&self, text: B) -> Distance {
        let mut state = self.initial_state();
        for &b in text.as_ref() {
            state = self.transition(state, b);
        }
        self.distance(state)
    }

    /// Returns the output distance associated to the current state
    pub fn distance(&self, state_id: u32) -> Distance {
        self.distances[state_id as usize]
    }


    /// Returns the number of states in the `DFA`.
    pub fn num_states(&self) -> usize {
        self.transitions.len()
    }

    /// Returns the destination state reached
    /// after consuming a given byte.
    pub fn transition(&self, from_state_id: u32, b: u8) -> u32 {
        self.transitions[from_state_id as usize][b as usize]
    }
}




pub struct DFAStateBuilder<'a> {
    dfa_builder: &'a mut DFABuilder,
    state_id: u32,
    default_successor: [u32; 4],
}

impl<'a> DFAStateBuilder<'a> {

    fn add_transition_id(&mut self, from_state_id: u32, b: u8, to_state_id: u32) {
        self.dfa_builder.transitions[from_state_id as usize][b as usize] = to_state_id;
    }

    pub fn add_transition(&mut self, chr: char, to_state_id: u32) {
        let mut buffer = [0u8; 4];
        let bytes: &[u8] = chr.encode_utf8(&mut buffer).as_bytes();
        let mut from_state_id_decoded = self.state_id;
        for (i, b) in bytes[..bytes.len() - 1].iter().cloned().enumerate() {
            let remaining_num_bytes = bytes.len() - i as usize - 1 as usize;
            let default_successor = self.default_successor[remaining_num_bytes];
            let mut intermediary_state_id_decoded: u32 = self.dfa_builder.transitions[from_state_id_decoded as usize][b as usize];
            if intermediary_state_id_decoded == default_successor {
                intermediary_state_id_decoded = self.dfa_builder.allocate();
                fill(&mut self.dfa_builder.transitions[intermediary_state_id_decoded as usize], default_successor);
            }
            self.add_transition_id(from_state_id_decoded, b, intermediary_state_id_decoded);
            from_state_id_decoded = intermediary_state_id_decoded;
        }

        let to_state_id_decoded = self.dfa_builder.get_or_allocate(Utf8State::Original(to_state_id));
        self.add_transition_id(from_state_id_decoded, bytes[bytes.len() - 1], to_state_id_decoded);
    }
}

pub struct DFABuilder {
    index: HashMap<Utf8State, u32>,
    distances: Vec<Distance>,
    transitions: Vec<[u32; 256]>,
    initial_state: u32,
    num_states: u32,
}

#[derive(Eq, PartialEq, Hash, Clone, Debug)]
enum Utf8State {
    Original(u32),
    Predecessor(u32, u8), // predecessor after n-bytes.
}

impl DFABuilder {

    pub fn with_capacity(capacity: usize) -> DFABuilder {
        DFABuilder {
            index: HashMap::with_capacity(capacity),
            distances: Vec::with_capacity(capacity),
            transitions: Vec::with_capacity(capacity),
            initial_state: 0u32,
            num_states: 0u32,
        }
    }

    fn allocate(&mut self) -> u32 {
        let new_state = self.num_states;
        self.num_states += 1;
        self.distances.resize(new_state as usize + 1, Distance::AtLeast(255));
        self.transitions.resize(new_state as usize + 1, [0u32; 256]);
        new_state
    }

    fn get_or_allocate(&mut self, state: Utf8State) -> u32 {
        if let Some(state) = self.index.get(&state) {
            return *state;
        }
        let new_state = self.allocate();
        self.index.insert(state, new_state);
        new_state
    }

    pub fn set_initial_state(&mut self, initial_state: u32) {
        let state_id_decoded = self.get_or_allocate(Utf8State::Original(initial_state));
        self.initial_state = state_id_decoded
    }

    pub fn add_state(&mut self, state: u32, distance: Distance, default_successor_orig: u32) -> DFAStateBuilder {
        let state_id = self.get_or_allocate(Utf8State::Original(state));
        self.distances[state_id as usize] = distance;
        let default_successor_id = self.get_or_allocate(Utf8State::Original(default_successor_orig));
        let mut predecessor_state_id = default_successor_id;
        let mut predecessor_states = [predecessor_state_id; 4];

        {
            for num_bytes in 1..4 {
                let predecessor_state = Utf8State::Predecessor(default_successor_id, num_bytes as u8);
                predecessor_state_id = self.get_or_allocate(predecessor_state);
                predecessor_states[num_bytes] = predecessor_state_id;
                let succ = predecessor_states[num_bytes - 1];
                for b in self.transitions[predecessor_state_id as usize].iter_mut() {
                    *b = succ;
                }
            }
        }

        {
            let transitions = &mut self.transitions[state_id as usize];
            for b in 0..192 {
                let last_state = predecessor_states[0];
                transitions[b as usize] = last_state;
            }
            for b in 192..224 {
                let last_state = predecessor_states[1];
                transitions[b as usize] = last_state;
            }
            for b in 224..240 {
                let last_state = predecessor_states[2];
                transitions[b as usize] = last_state;
            }
            for b in 240..256 {
                let last_state = predecessor_states[3];
                transitions[b as usize] = last_state;
            }
        }

        DFAStateBuilder {
            dfa_builder: self,
            state_id: state_id,
            default_successor: predecessor_states,
        }
    }

    pub fn build(self) -> DFA {
        DFA {
            transitions: self.transitions,
            distances: self.distances,
            initial_state: self.initial_state
        }
    }
}

#[cfg(test)]
mod tests {

    use super::DFABuilder;
    use super::Distance;


    #[test]
    fn test_utf8_dfa_builder() {
        let mut dfa_builder = DFABuilder::with_capacity(2);
        dfa_builder
            .add_state(0, Distance::Exact(1u8), 1);
        dfa_builder
            .add_state(1, Distance::Exact(0u8), 0);
        dfa_builder.set_initial_state(1u32);
        let dfa = dfa_builder.build();
        let parity_num_letters = |s: &str| {
            dfa.eval(s).to_u8()
        };

        assert_eq!(parity_num_letters("abcdef"), 0u8);
        assert_eq!(parity_num_letters("a"), 1u8);
        assert_eq!(parity_num_letters("aあ"), 0u8);
        assert_eq!(parity_num_letters("❤"), 1u8);
        assert_eq!(parity_num_letters("❤❤"), 0u8);
        assert_eq!(parity_num_letters("❤a"), 0u8);
        assert_eq!(parity_num_letters("あ"), 1u8);
        assert_eq!(parity_num_letters("ああ"), 0u8);
    }
}