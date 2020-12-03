from flask import Flask, render_template, request, make_response, g
from redis import Redis
import os
import socket
import random
import json

app = Flask(__name__)

def get_redis():
    if not hasattr(g, 'redis'):
        g.redis = Redis(host="queue", db=0, socket_timeout=5)
    return g.redis

@app.route('/', methods=['GET'])
def index():
    return "hello!"

@app.route("/vote", methods=['POST'])
def private():
    #db stuff
    voter_id = request.cookies.get('voter_id')
    if not voter_id:
        voter_id = hex(random.getrandbits(64))[2:-1]

    vote = None

    if request.method == 'POST':
        redis = get_redis()
        vote = request.form['vote']
        data = json.dumps({'voter_id': voter_id, 'vote': vote})
        redis.rpush('votes', data)


if __name__ == "__main__":
    app.run(host='0.0.0.0', port=3000, threaded=True)