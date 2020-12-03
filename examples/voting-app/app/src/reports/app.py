from flask import Flask
#from flask_pymongo import PyMongo

app = Flask(__name__)

@app.route('/', methods=['GET'])
def index():
    return "hello!"

@app.route("/results", methods=['GET'])
def private():
    #db stuff
    return "connect too db"

if __name__ == "__main__":
    app.run(host='0.0.0.0', port=3000)
