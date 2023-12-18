template = """coord = {}
ship_coords = {}
c = {}
"""


def quote(x):
    return '"{}"'.format(x)

def dirname(n):
    return "tests/" + str(n)

def gen(n):
    # remove directory if it exists
    import os
    import shutil
    if os.path.exists(dirname(n)):
        shutil.rmtree(dirname(n))
    # copy "template" directory to newly created directory
    shutil.copytree("template", dirname(n))
    
    # generate Prover.toml
    coord = quote(0)
    ship_coords = str([str(0) for _ in range(n)]).replace("'", '"')
    c = quote(n)
    with open("{}/Prover.toml".format(dirname(n)), "w") as f:
        f.write(template.format(coord, ship_coords, c))

    with open("{}/src/main.nr".format(dirname(n))) as f:
        nr = f.read()
    
    with open("{}/src/main.nr".format(dirname(n)), "w") as f:
        f.write(nr.replace("TBD", str(n)))

# call gen with n = passed in argument
import sys
if __name__ == "__main__":
    for i in [256, 512, 1024, 2048, 4096, 8192]:
        gen(i)