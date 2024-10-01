# List SQS queue status

My motivation for creating this tool comes from my current workplace's
vigorous use of AWS Simple Queue Service, SQS. The AWS administration
console does not make it very easy to find which queues currently
contain messages. This tool looks through all of the SQS queues in an
AWS account and, by default, lists the queues that contain any messages.

```bash
% sqs-status --help
List the status of AWS SQS queues.

You can set the environment variable `RUST_LOG` to adjust logging,
for example `RUST_LOG=trace sqs-status`.

Usage: sqs-status [OPTIONS]

Options:
  -a, --all
          Print all SQS queues instead of only non-empty queues

  -p, --profile <PROFILE>
          AWS profile to use.

          This overrides the standard (and complex!) AWS profile
          handling.

  -r, --region <REGION>
          AWS region to target.

          This override the standard (and complex!) AWS region
          handling.

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```
