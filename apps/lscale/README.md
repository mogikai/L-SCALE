# L-SCALE

L-SCALE is an experimental state learning approach utilizing locality-sensitive hashing on coverage feedback as
observations for the L* algorithm.
This project contains the main binary of the approach.
For details on the approach refer to the paper.

## Command line completions

We provide command line completion for our binary.
Run

## Comands used for evaluation

We used the following commands (started from the root directory) to generate the artifacts we use in our paper:

Open62541 small corpus evaluation

```shell
MOGI_LOG=mogi_lscale=info ./lscale evaluate --context=fuzzer/assets/opcua --corpus=small --binary=/home/user/open62541_server_fuzzing_fixed --output=.output/opcua/ --system-root=emulator/systems/ubuntu2310 --threshold-start=0 --threshold-end=40 --timeout=14400
```

Open62541 large corpus evaluation

```shell
MOGI_LOG=mogi_lscale=info ./lscale evaluate --context=fuzzer/assets/opcua --corpus=large --binary=/home/user/open62541_server_fuzzing_fixed --output=.output/opcua/ --system-root=emulator/systems/ubuntu2310 --threshold-start=0 --threshold-end=40 --timeout=14400
```

Open62541 small corpus with refinement evaluation

```shell
MOGI_LOG=mogi_lscale=info ./lscale evaluate --context=fuzzer/assets/opcua --corpus=small --binary=/home/user/open62541_server_fuzzing_fixed --output=.output/opcua/ --system-root=emulator/systems/ubuntu2310 --threshold-start=0 --threshold-end=40 --timeout=14400 --do-refinement=20
```

Open62541 large corpus with refinement evaluation

```shell
MOGI_LOG=mogi_lscale=info ./lscale evaluate --context=fuzzer/assets/opcua --corpus=large --binary=/home/user/open62541_server_fuzzing_fixed --output=.output/opcua/ --system-root=emulator/systems/ubuntu2310 --threshold-start=0 --threshold-end=40 --timeout=14400 --do-refinement=20
```