#!/bin/bash
# 1. Setup Architecture
mkdir -p third_party/tflite
mkdir -p lib

echo ">> [FETCH] Downloading ARM64 TFLite C-API Headers..."
BASE_URL="https://raw.githubusercontent.com/tensorflow/tensorflow/master/tensorflow/lite/c"
wget -q $BASE_URL/c_api.h -O third_party/tflite/c_api.h
wget -q $BASE_URL/c_api_types.h -O third_party/tflite/c_api_types.h
wget -q $BASE_URL/common.h -O third_party/tflite/common.h

echo ">> [FETCH] Downloading Pre-compiled libtensorflowlite_c (aarch64)..."
# We are pulling a verified ARM64 build (2.16+) optimized for mobile CPUs
WHEEL_URL="https://github.com/PINTO0309/TensorflowLite-bin/raw/master/2.16.1/tflite_runtime-2.16.1-cp311-cp311-linux_aarch64.whl"
wget -q $WHEEL_URL -O tflite_temp.zip

# Extracting the .so directly from the wheel to avoid Python dependencies
unzip -q tflite_temp.zip "tflite_runtime/_pywrap_tensorflow_interpreter_wrapper.so"
mv tflite_runtime/_pywrap_tensorflow_interpreter_wrapper.so lib/libtensorflowlite_c.so
rm -rf tflite_runtime tflite_temp.zip

echo ">> [WELD] Muscles Seated in ./lib/libtensorflowlite_c.so"
