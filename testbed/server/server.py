from flask import Flask
app = Flask(__name__)

@app.route("/")
def index():
    return "response!"

@app.route("/private")
def private():
    return "private area"

@app.route("/hello/<string:name>")
def hello(name):
    return name

if __name__ == "__main__":
    app.run(host='0.0.0.0', port=80)
