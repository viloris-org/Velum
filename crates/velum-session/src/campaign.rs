//! Seeded transition campaign for the deterministic session tracer.

use crate::{
    Acknowledgement, CarrierDisposition, FlowLimits, InMemoryCarrier, ReceiveResult, SessionTracer,
};
use velum_protocol::{Epoch, Sequence};

const TRIALS: u64 = 10_000;

#[derive(Debug)]
struct Generator(u64);

impl Generator {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
        self.0
    }

    fn bytes(&mut self) -> Vec<u8> {
        let len = (self.next() % 16 + 1) as usize;
        (0..len).map(|_| self.next() as u8).collect()
    }
}

fn session() -> SessionTracer {
    SessionTracer::new(
        Epoch(0),
        FlowLimits {
            max_flows: 1,
            max_pending_segments: 8,
            max_pending_bytes: 128,
            max_pending_age: 16,
            max_events: 16,
        },
    )
}

fn disposition(random: u64) -> CarrierDisposition {
    match random % 4 {
        0 => CarrierDisposition::Drop,
        1 => CarrierDisposition::Duplicate,
        2 => CarrierDisposition::Delay(1),
        _ => CarrierDisposition::Deliver,
    }
}

fn deliver(receiver: &mut SessionTracer, segments: Vec<crate::Segment>, output: &mut Vec<u8>) {
    for segment in segments {
        if let ReceiveResult::Delivered(bytes) =
            receiver.receive(segment).expect("valid flow and epoch")
        {
            output.extend(bytes);
        }
    }
}

fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

fn run_trial(seed: u64) -> u64 {
    let mut random = Generator::new(seed);
    let mut sender = session();
    let mut receiver = session();
    let sender_flow = sender.open_reliable_flow().expect("open sender flow");
    receiver.open_reliable_flow().expect("open receiver flow");
    let mut carrier = InMemoryCarrier::new();
    let mut expected = Vec::new();
    let mut delivered = Vec::new();
    let transition_after = (random.next() % 3 + 1) as usize;

    for index in 0..4 {
        let bytes = random.bytes();
        expected.extend_from_slice(&bytes);
        let segment = sender.send(sender_flow, bytes).expect("within limits");
        if index + 1 == transition_after {
            assert_eq!(sender.begin_transition(), Epoch(1));
            assert_eq!(receiver.begin_transition(), Epoch(1));
        }
        if random.next() & 1 == 0 {
            carrier.set_available(false);
            carrier.transmit(segment.clone(), CarrierDisposition::Deliver);
            carrier.set_available(true);
        }
        carrier.transmit(segment, disposition(random.next()));
        deliver(
            &mut receiver,
            carrier.advance(random.next() % 2),
            &mut delivered,
        );
    }

    // A recovered carrier reissues session-owned unacknowledged bytes using
    // the current epoch while retaining their original logical sequences.
    let mut recovery = InMemoryCarrier::new();
    for segment in sender
        .resume_unacknowledged(sender_flow)
        .expect("known flow")
    {
        recovery.transmit(segment, CarrierDisposition::Deliver);
    }
    deliver(&mut receiver, recovery.advance(0), &mut delivered);
    sender
        .acknowledge(Acknowledgement {
            flow_id: sender_flow,
            epoch: Epoch(0),
            through: Sequence((transition_after - 1) as u64),
        })
        .expect("retiring epoch acknowledgement");
    receiver.complete_transition();
    sender
        .acknowledge(Acknowledgement {
            flow_id: sender_flow,
            epoch: sender.epoch(),
            through: Sequence(3),
        })
        .expect("logical acknowledgement");

    assert_eq!(delivered, expected, "seed {seed} changed delivered bytes");
    assert!(sender.pending(sender_flow).expect("known flow").is_empty());
    checksum(&delivered)
}

#[test]
fn ten_thousand_seeded_transitions_preserve_byte_exact_delivery() {
    let campaign_checksum = (0..TRIALS).fold(0_u64, |checksum, seed| {
        checksum.wrapping_add(run_trial(seed))
    });

    assert_eq!(campaign_checksum, 4_550_704_779_471_716_960);
}
