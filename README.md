# deluge-kit-generator

A CLI tool to create Synthstrom Deluge kits.  
Use `--help` for the documentation.

## Installation

To install `deluge-kit-generator`, you will need to have Rust installed on your computer. Once you have Rust installed, you can install `deluge-kit-generator` by running the following command:

```
cargo install deluge-kit-generator
```

## Command `from-regions`
This command creates a kit for each samples with the rows specified by the regions. Use an audio editor like Ocenaudio to create the regions.

### Option `--combine-all`
When this option is selected, a single patch kit for the Synthstrom Deluge is created, containing all the regions from all the samples. This allows for greater versatility in patch creation and simplifies the organization of the sample library within the Deluge.

