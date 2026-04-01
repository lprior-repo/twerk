# Examples

Real-world workflow examples.

## CI/CD Pipeline

```yaml
name: ci-pipeline
inputs:
  repo_url: https://github.com/example/app
  image_tag: latest
defaults:
  retry:
    limit: 2
  timeout: 30m
tasks:
  - name: checkout
    var: repo_path
    image: alpine/git:latest
    run: |
      git clone {{ inputs.repo_url }} /app
      echo "/app" > $TWERK_OUTPUT

  - name: test
    image: node:20
    env:
      REPO: '{{ tasks.checkout }}'
    run: |
      cd $REPO
      npm ci
      npm test

  - name: build image
    image: docker:latest
    env:
      TAG: '{{ inputs.image_tag }}'
    run: |
      docker build -t myapp:$TAG $REPO
      docker push myapp:$TAG

  - name: deploy
    if: "{{ job.state == 'COMPLETED' }}"
    image: alpine:latest
    run: |
      echo "Deployed myapp:{{ inputs.image_tag }}"
```

## Video Processing

```yaml
name: process video
inputs:
  video_url: https://example.com/video.mov
  resolutions: [480, 720, 1080]
tasks:
  - name: download video
    var: video_path
    image: alpine:latest
    run: |
      wget "{{ inputs.video_url }}" -O /tmp/input.mov
      echo "/tmp/input.mov" > $TWERK_OUTPUT

  - name: transcode
    each:
      list: '{{ inputs.resolutions }}'
      concurrency: 3
      var: "output_{{ item.value }}"
      task:
        image: jrottenberg/ffmpeg:3.4-alpine
        env:
          RES: '{{ item.value }}'
          INPUT: '{{ tasks.download_video }}'
        run: |
          ffmpeg -i $INPUT -vf "scale=-2:$RES" /tmp/output_$RES.mp4
          echo "/tmp/output_$RES.mp4" > $TWERK_OUTPUT

  - name: upload
    image: amazon/aws-cli:latest
    env:
      BUCKET: my-videos
    run: |
      aws s3 cp {{ tasks.transcode.outputs.output_480 }} s3://$BUCKET/video_480.mp4
      aws s3 cp {{ tasks.transcode.outputs.output_720 }} s3://$BUCKET/video_720.mp4
      aws s3 cp {{ tasks.transcode.outputs.output_1080 }} s3://$BUCKET/video_1080.mp4
```

## Data ETL Pipeline

```yaml
name: etl-pipeline
inputs:
  db_host: prod-db.example.com
  db_name: analytics
secrets:
  db_user: readonly_user
  db_password: secret
tasks:
  - name: extract
    var: dump_file
    image: postgres:15
    run: |
      PGPASSWORD={{ secrets.db_password }} \
      pg_dump -h {{ inputs.db_host }} -U {{ secrets.db_user }} \
        -d {{ inputs.db_name }} > /tmp/dump.sql
      gzip -c /tmp/dump.sql > /tmp/dump.gz
      echo "/tmp/dump.gz" > $TWERK_OUTPUT

  - name: transform
    var: metrics
    image: python:3.11
    files:
      transform.py: |
        import gzip, json
        with gzip.open('/tmp/dump.gz', 'rt') as f:
            data = f.read()
        result = {"records": len(data.split())}
        print(json.dumps(result))
    run: python transform.py > $TWERK_OUTPUT

  - name: load
    image: alpine:latest
    env:
      METRICS: '{{ tasks.transform }}'
    run: |
      echo "Loading $METRICS to warehouse"
```

## Scheduled Backup

```yaml
name: nightly-backup
schedule:
  cron: "0 2 * * *"
tasks:
  - name: backup postgres
    var: backup_file
    image: postgres:15
    run: |
      PGPASSWORD=$PGPASSWORD pg_dump -h $DB_HOST -U $DB_USER $DB_NAME \
        | gzip > /backups/db_$(date +%Y%m%d).sql.gz
      echo "/backups/db_$(date +%Y%m%d).sql.gz" > $TWERK_OUTPUT

  - name: upload to s3
    image: amazon/aws-cli:latest
    env:
      BACKUP: '{{ tasks.backup_postgres }}'
    run: |
      aws s3 cp $BACKUP s3://my-backups/

  - name: notify
    image: alpine:latest
    webhooks:
      - url: https://hooks.example.com/backup
        event: task.StateChange
        if: "{{ task.state == 'COMPLETED' }}"
    run: |
      echo "Backup completed"
```

## Parallel Execution

```yaml
name: parallel computation
tasks:
  - name: setup
    var: items
    image: alpine:latest
    run: |
      echo '[1,2,3,4,5]' > $TWERK_OUTPUT

  - name: process in parallel
    parallel:
      tasks:
        - name: task a
          image: alpine:latest
          run: echo "A done"
        - name: task b
          image: alpine:latest
          run: sleep 2 && echo "B done"
        - name: task c
          image: alpine:latest
          run: echo "C done"

  - name: collect
    image: alpine:latest
    run: echo "All parallel tasks complete"
```

## GPU Workload

```yaml
name: ml inference
tasks:
  - name: download model
    var: model_path
    image: alpine:latest
    run: |
      wget https://example.com/model.pt -O /tmp/model.pt
      echo "/tmp/model.pt" > $TWERK_OUTPUT

  - name: inference
    image: pytorch:latest
    gpus: all
    env:
      MODEL: '{{ tasks.download_model }}'
    run: |
      python inference.py --model $MODEL
```

## Conditional Execution

```yaml
name: conditional workflow
inputs:
  environment: production
tasks:
  - name: validate
    var: validation_result
    image: alpine:latest
    run: |
      echo "validation complete" > $TWERK_OUTPUT

  - name: deploy to prod
    if: "{{ inputs.environment == 'production' }}"
    image: alpine:latest
    run: echo "Deploying to production!"

  - name: deploy to staging
    if: "{{ inputs.environment == 'staging' }}"
    image: alpine:latest
    run: echo "Deploying to staging!"
```

## See Also

- [Jobs](jobs.md) — Job reference
- [Tasks](tasks.md) — Task reference
- [REST API](rest-api.md) — API reference
