# Operator Interview Records

Stage 0 requires at least five interviews with operators who maintain multiple
tunnel protocols or transport configurations. Store one Markdown record per
interview using a non-identifying ID such as `operator-001.md`. Do not commit
names, credentials, endpoint addresses, or customer data.

Each record must contain:

```markdown
# Operator NNN

- Date:
- Interviewer:
- Operator context:
- Protocols or transport configurations maintained:
- Networks and regions encountered:

## Recent Failure

- What changed in the network?
- How was the failure detected?
- What did the operator do?
- Which application sessions were interrupted?
- How long did useful recovery take?

## Problem Ranking

Rank continuity, configuration effort, throughput, client ecosystem, operating
cost, and camouflage/deployment concerns. Record the operator's own wording.

## Current Workarounds

- Manual or automatic selection behavior:
- Warm standby cost, if any:
- Observability available during failure:

## Falsification

- Would preserving an existing flow materially help?
- When is application reconnect already good enough?
- What would make Velum not worth deploying?

## Evidence Classification

- Material reconnect or manual switching problem: yes | no | unclear
- Permission to quote anonymously: yes | no
- Follow-up needed:
```

Summaries may update the evidence ledger only after the interviewee's actual
experience is separated from interviewer inference. The Stage 0 exit gate needs
three `yes` records; five completed interviews alone are insufficient.
