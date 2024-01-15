template = """a = {}
"""
TOTAL = 9

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
    a = str([str(i % TOTAL + 1) for i in range(n)]).replace("'", '"')
    with open("{}/Prover.toml".format(dirname(n)), "w") as f:
        f.write(template.format(a))

    with open("{}/src/main.nr".format(dirname(n))) as f:
        nr = f.read()
    
    with open("{}/src/main.nr".format(dirname(n)), "w") as f:
        f.write(nr.replace("TBD", str(n)))

# call gen with n = passed in argument
import sys
if __name__ == "__main__":
    for i in range(6, 11+1):
        gen(2**i)