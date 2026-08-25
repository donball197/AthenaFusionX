<?php
$con = mysqli_connect("localhost", "root", "", "test");
if (!$con) { die("Connection failed"); }
if (!empty($_REQUEST['uid'])) {
    $username = $_REQUEST['uid'];
    $pass = $_REQUEST['password'];
    $q = "SELECT * FROM users where username='".$username."' AND password = '".md5($pass)."'" ;
    if (!mysqli_query($con,$q)) {
        echo 'Error: ' . mysqli_error($con);
    }else{
        $result = mysqli_query($con,$q);
        $row = mysqli_fetch_array($result);
        if ($row){ $_SESSION["username"] = $row[1]; }
    }
}
?>
