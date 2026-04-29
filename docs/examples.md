# Twerk Workflow Examples

Status: draft

Audience: workflow authors, UI builders, action authors, and AI agents.

## Purpose

This document collects canonical workflow examples for the v1 language. Examples should stay small enough to understand and strict enough to validate.

Each example demonstrates a specific language feature.

## 1. Manual Greeting

Feature: `manual`, `inputs`, `set`, `result`.

```yaml
version: twerk/v1
name: hello_world

when:
  manual: {}

inputs:
  name: text

steps:
  - id: greeting
    set:
      message: "Hello $input.name"

result:
  message: $greeting.message

examples:
  - name: ada
    input:
      name: Ada
    expect:
      result:
        message: Hello Ada
```

## 2. Webhook Issue Triage

Feature: webhook input mapping, uniqueness, action call, `choose`, tagged result.

```yaml
version: twerk/v1
name: issue_triage

when:
  webhook:
    method: POST
    path: /hooks/issues
    unique: request.header.X-Delivery-ID

inputs:
  delivery_id:
    from: request.header.X-Delivery-ID
    is: text
  title:
    from: request.body.issue.title
    is: text
  body:
    from: request.body.issue.body
    is: text
    default: ""

steps:
  - id: classify
    do: ai.classify
    with:
      text: "$input.title\n\n$input.body"
      labels:
        - bug
        - question
        - feature

  - id: route
    choose:
      - if: $classify.label == "bug"
        steps:
          - id: bug_ticket
            do: ticket.create
            with:
              title: $input.title
              body: $input.body
              priority: high
        result:
          kind: bug
          ticket_id: $bug_ticket.id
          priority: high

      - otherwise: true
        steps:
          - id: normal_ticket
            do: ticket.create
            with:
              title: $input.title
              body: $input.body
              priority: normal
        result:
          kind: normal
          ticket_id: $normal_ticket.id
          priority: normal

result:
  delivery_id: $input.delivery_id
  label: $classify.label
  ticket_id: $route.ticket_id
  priority: $route.priority
```

## 3. Scheduled Digest

Feature: schedule trigger, typed vars, action chaining.

```yaml
version: twerk/v1
name: daily_digest

when:
  schedule:
    cron: "0 9 * * *"

vars:
  channel: ops-digest

steps:
  - id: load_items
    do: ticket.search
    with:
      query: status:open updated:24h

  - id: summarize
    do: ai.summarize
    with:
      items: $load_items.items

  - id: post_digest
    do: slack.message.send
    with:
      channel: $vars.channel
      text: $summarize.text

result:
  posted: $post_digest.ok
  count: $load_items.count
```

## 4. Parallel Enrichment

Feature: `together`, deterministic branch outputs, partial failure collection.

```yaml
version: twerk/v1
name: customer_enrichment

when:
  event:
    name: customer.created

inputs:
  email:
    from: event.body.email
    is: text
  customer_id:
    from: event.body.customer_id
    is: text

steps:
  - id: enrich
    together:
      fail: collect
      branches:
        profile:
          do: profile.lookup
          with:
            email: $input.email
        orders:
          do: order.list
          with:
            customer_id: $input.customer_id

  - id: summary
    set:
      profile_ok: $enrich.profile.ok
      orders_ok: $enrich.orders.ok

result:
  profile_ok: $summary.profile_ok
  orders_ok: $summary.orders_ok
```

## 5. Fan-Out Notifications

Feature: `for_each`, throttling, ordered output.

```yaml
version: twerk/v1
name: notify_customers

when:
  manual: {}

inputs:
  subject: text
  customers:
    is: list
    of:
      is: object
      fields:
        email: text
        name: text

steps:
  - id: send_all
    for_each:
      in: $input.customers
      as: customer
      at_once: 20
      per_second: 50
      do: email.send
      with:
        to: $customer.email
        subject: $input.subject
        body: "Hello $customer.name"
      try_again:
        times: 3
        wait:
          type: exponential
          initial: 1s
          max: 30s

result:
  sends: $send_all
```

## 6. Bounded API Pagination

Feature: `collect`, cursor pagination, hard limits.

```yaml
version: twerk/v1
name: collect_customers

when:
  manual: {}

secrets:
  api_token:
    required: true

steps:
  - id: customers
    collect:
      cursor:
        start: null
        next: $page.body.next_cursor

      page:
        do: http.get
        with:
          url: https://api.example.com/customers
          headers:
            Authorization: "Bearer $secrets.api_token"
          query:
            cursor: $cursor
            limit: 100

      items: $page.body.customers
      stop: $page.body.next_cursor == null

      limit:
        pages: 500
        items: 50000
        time: 5m
        wait_between: 100ms

  - id: totals
    reduce:
      in: $customers.items
      as: customer
      start:
        count: 0
        active_customers: []
      set:
        count: $total.count + 1
        active_customers: append_if($total.active_customers, $customer, $customer.active == true)

result:
  count: $totals.count
  active: length($totals.active_customers)
```

## 7. Bounded Polling

Feature: `repeat`, durable polling without arbitrary cycles.

```yaml
version: twerk/v1
name: wait_for_external_job

when:
  manual: {}

inputs:
  payload: object

steps:
  - id: create_job
    do: api.job.create
    with:
      payload: $input.payload

  - id: poll_job
    repeat:
      do: http.get
      with:
        url: "https://api.example.com/jobs/$create_job.id"
      until: $attempt.body.status == "done"
      limit:
        times: 120
        time: 20m
        wait_between: 10s

result:
  job_id: $create_job.id
  status: $poll_job.body.status
```

## 8. Human Approval

Feature: `ask`, conditional deploy, finish terminals.

```yaml
version: twerk/v1
name: production_deploy

when:
  manual: {}

inputs:
  service: text
  version: text

secrets:
  deploy_token:
    required: true

steps:
  - id: approval
    ask:
      to:
        role: deploy_approver
      question: "Deploy $input.service version $input.version to production?"
      choices:
        - approve
        - reject
      timeout: 24h

  - id: deploy
    if: $approval.answer == "approve"
    do: deploy.release
    with:
      token: $secrets.deploy_token
      service: $input.service
      version: $input.version
    then: done

  - id: rejected
    if: $approval.answer == "reject"
    finish:
      status: cancelled
      error: approval_rejected

  - id: done
    finish: success

result:
  answer: $approval.answer
```

## 9. Error Recovery

Feature: `try_again`, `on_error.then`, explicit recovery step.

```yaml
version: twerk/v1
name: safe_webhook_delivery

when:
  event:
    name: delivery.requested

inputs:
  url:
    from: event.body.url
    is: text
  payload:
    from: event.body.payload
    is: object

steps:
  - id: deliver
    do: http.post
    with:
      url: $input.url
      json: $input.payload
    try_again:
      times: 4
      when:
        - Http.Timeout
        - Http.RateLimited
      wait:
        type: exponential
        initial: 1s
        max: 30s
    on_error:
      then: record_failure

  - id: success
    finish: success

  - id: record_failure
    do: delivery.failure.record
    with:
      url: $input.url
      error: $error.code
    then: failed

  - id: failed
    finish:
      status: failure
      error: delivery_failed

result:
  url: $input.url
```

## 10. Webhook Reply

Feature: fast webhook response action.

```yaml
version: twerk/v1
name: webhook_reply_example

when:
  webhook:
    method: POST
    path: /reply
    unique: request.header.X-Request-ID

inputs:
  request_id:
    from: request.header.X-Request-ID
    is: text
  body:
    from: request.body
    is: object

steps:
  - id: accepted
    do: webhook.reply
    with:
      status: 202
      json:
        ok: true
        request_id: $input.request_id

  - id: process
    do: worker.process
    with:
      body: $input.body

result:
  request_id: $input.request_id
  processed: $process.ok
```

Runtime policy decides whether `webhook.reply` is an early response action or a final response action. The workflow history must make the behavior visible.

## Example Validation Checklist

Every example should pass:

- Restricted YAML profile.
- Closed top-level schema.
- Closed step schema.
- Global step ID uniqueness.
- No raw runtime references outside `inputs.from`.
- No undeclared secrets.
- No skipped-step unsafe references.
- Action `with` schemas.
- Result secret taint checks.
- Payload and nesting limits.
