# Preview Environments

Every PR gets its own isolated environment — database, cache, compute, everything.

## GitHub Actions

```yaml
# .github/workflows/preview.yml
name: Preview
on:
  pull_request:
    types: [opened, synchronize]

jobs:
  preview:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Deploy preview
        run: |
          cooper deploy \
            --env preview-pr-${{ github.event.number }} \
            --cloud aws \
            --auto-destroy-after 48h

          URL=$(cooper env url preview-pr-${{ github.event.number }})
          echo "Preview: $URL" >> $GITHUB_STEP_SUMMARY
```

## What you get

- Isolated Postgres with your migrations applied
- Isolated cache
- Isolated queues and topics
- Own URL: `https://preview-pr-42.your-domain.com`
- Auto-destroys after 48h

## Cleanup

```bash
cooper destroy --env preview-pr-42
```

Or let `--auto-destroy-after` handle it.
