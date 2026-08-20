import os
import subprocess

def ping_server(user_ip_address):
    # DANGER: Directly concatenating user input into an OS command
    os.system("ping -c 4 " + user_ip_address)
