import sys

sys.stdout.write("What is your name?\n")
sys.stdout.flush()
name = sys.stdin.readline().strip()
sys.stdout.write("Hello, " + name + "!\n")
sys.stdout.flush()

sys.stdout.write("Enter a number:\n")
sys.stdout.flush()
num = int(sys.stdin.readline().strip())
sys.stdout.write("Double: " + str(num * 2) + "\n")
sys.stdout.write("Triple: " + str(num * 3) + "\n")
sys.stdout.flush()

sys.stdout.write("Enter a word:\n")
sys.stdout.flush()
word = sys.stdin.readline().strip()
sys.stdout.write("You said: " + word + "\n")
sys.stdout.write("Done!\n")
sys.stdout.flush()
