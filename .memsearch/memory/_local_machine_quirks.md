# Local Machine Quirks

Use this for machine-local behavior that should not be mistaken for repo contract.

## Memsearch

- `.memsearch/.last_msg_time` and `.memsearch/memory/*.md` are local memory/session artifacts and should not be included in normal code pushes.
- Daily memory files can contain both a curated section and an auto-captured session log.

## Local Testing Notes

- Long-running background processes may need explicit cleanup after manual QA runs.
- Process-global environment changes can leak across parallel tests if they are not locked or restored.

## Update Rules

- Keep these notes machine-local and operational.
- Do not mix them with product behavior or repo-wide contracts.
