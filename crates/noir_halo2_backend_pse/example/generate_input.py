import json
import random
import os
import argparse

def decimal_string_to_binary_list(decimal_str, n):
    binary_str = bin(int(decimal_str))[2:]
    padding = n - len(binary_str)
    binary_list = list(binary_str)
    binary_list.reverse()
    return binary_list + list(["0" for _ in range(padding)])

def generate_json_file(directory):
    os.makedirs(directory, exist_ok=True)
    filename = os.path.join(directory, "input.json")
    if directory == "MultiAND":
        data = {"in": [random.choice(["0", "1"]) for _ in range(4096)]}
    elif directory == "BigSub":
        data = {
            "a": [random.choice(["0", "1"]) for _ in range(253)],
            "b": [random.choice(["0", "1"]) for _ in range(253)]
        }
    elif directory == "BigLessThan":
        data = {
            "a": [random.choice(["0", "1"]) for _ in range(253)],
            "b": [random.choice(["0", "1"]) for _ in range(253)]
        }
    elif directory == "BigIsEqual":
        data = {
            "in": [
                [random.choice(["0", "1"]) for _ in range(1000)] for _ in range(2)
            ],
        }
    elif directory == "MultiMux":
        data = {
            "c": [
                [random.choice(["0", "1"]) for _ in range(2)] for _ in range(4096)
            ],
            "s": "1",
        }
    elif directory == "Decoder":
        data = {
            "inp": "2"
        }
    elif directory == "BigAddNoCarry":
        data = {
            "a": [random.choice(["0", "1"]) for _ in range(253)],
            "b": [random.choice(["0", "1"]) for _ in range(253)],
        }
    elif directory == "BinSum":
        ops = 100
        n = 64
        data = {"in":[
                [random.choice(["0", "1"]) for _ in range(n)] for _ in range(ops)
            ],
        }
    elif directory == "num2bits":
        in_str = "4444444444"
        n = 2048
        data = {
            "in": in_str,
            "expected": "0"
            # decimal_string_to_binary_list(in_str, n)
        }

    with open(filename, "w") as f:
        json.dump(data, f, indent=4)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Generate JSON file with binary sequences.")
    parser.add_argument("directory", type=str, help="Directory path to save input.json")
    args = parser.parse_args()
    generate_json_file(args.directory)
    os.chdir(args.directory)
    os.system("circom circuit.circom --r1cs --wasm --sym --c")
    os.chdir("circuit_js")
    os.system("cp ../input.json .")
    os.system("node generate_witness.js circuit.wasm input.json witness.wtns")
    os.chdir("..")