#!/bin/bash

set -e

SKYBOX_DIR=$(dirname $0)
INFILE=$1
if [ -z $INFILE ]; then
    echo "Usage: $0 <input_file>"
    exit 1
fi

DIFFUSE_OUTFILE="${INFILE%.*}_diffuse.ktx2"
SPECULAR_OUTFILE="${INFILE%.*}_specular.ktx2"
LUT_OUTFILE="${INFILE%.*}_LUT.png"

if [ -z $GLTF_IBL_SAMPLER_PATH ]; then
    GLTF_IBL_SAMPLER_PATH="$HOME/.local/bin/gltf-ibl-sampler"
fi

if [ ! -x $GLTF_IBL_SAMPLER_PATH ]; then
    echo "Error: GLTF IBL Sampler CLI Tool not found"
    echo "Please set GLTF_IBL_SAMPLER_PATH to the path of the GLTF IBL Sampler CLI binary"
    exit 1
fi

echo "Generating skybox assets from $INFILE to $SKYBOX_DIR"
echo "Using GLTF IBL Sampler at $GLTF_IBL_SAMPLER_PATH"

mkdir -p $SKYBOX_DIR

# generate diffuse texture
$GLTF_IBL_SAMPLER_PATH \
    -inputPath $INFILE \
    -outCubeMap $SKYBOX_DIR/$DIFFUSE_OUTFILE \
    -distribution Lambertian \
    -mipLevelCount 1 \
    -sampleCount 4096 \
    -cubeMapResolution 32 \
    -targetFormat R16G16B16A16_SFLOAT \
    -outLUT $SKYBOX_DIR/$LUT_OUTFILE

# discard the pipeline.cache file
rm -f pipeline.cache

# discard the LUT for the diffuse texture
rm -f $SKYBOX_DIR/$LUT_OUTFILE

# generate specular texture
$GLTF_IBL_SAMPLER_PATH \
    -inputPath $INFILE \
    -outCubeMap $SKYBOX_DIR/$SPECULAR_OUTFILE \
    -distribution GGX \
    -mipLevelCount 5 \
    -sampleCount 4096 \
    -cubeMapResolution 512 \
    -targetFormat R16G16B16A16_SFLOAT \
    -outLUT $SKYBOX_DIR/$LUT_OUTFILE

# discard the pipeline.cache file
rm -f pipeline.cache

echo "Done"
exit 0
